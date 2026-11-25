use super::{AppState, driver_for, ensure_writable};
use crate::models::{
    ColumnInfo, ColumnValue, GeneratePreview, GenerateProgress, GenerateRequest, GenerateSummary,
    GeneratorColumn, PendingTableChange,
};
use chrono::{Duration, NaiveDate};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};

const MAX_ROWS: usize = 10_000;
const MAX_TEXT: usize = 10_000;

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.state = value;
        value.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: u64) -> u64 {
        if bound <= 1 {
            0
        } else {
            self.next_u64() % bound
        }
    }

    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range_i64(&mut self, low: i64, high: i64) -> i64 {
        if high <= low {
            return low;
        }
        let span = (high as i128 - low as i128) as u128;
        low + (self.next_u64() as u128 % span) as i64
    }
}

#[derive(Debug)]
struct ColumnSpec {
    info: ColumnInfo,
    config: GeneratorColumn,
    sequence: i64,
    is_datetime: bool,
}

fn is_binary(data_type: &str) -> bool {
    let upper = data_type.to_ascii_uppercase();
    [
        "BLOB",
        "BINARY",
        "VARBINARY",
        "BYTEA",
        "BIT",
        "IMAGE",
        "RAW",
        "GEOMETRY",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
}

fn is_datetime_type(data_type: &str) -> bool {
    let upper = data_type.to_ascii_uppercase();
    upper.contains("TIMESTAMP") || upper.contains("DATETIME")
}

fn build_specs(
    columns: &[ColumnInfo],
    request: &GenerateRequest,
) -> Result<(Vec<ColumnSpec>, Vec<String>), String> {
    if request.count == 0 || request.count > MAX_ROWS {
        return Err(format!("生成行数必须在 1-{MAX_ROWS} 之间"));
    }
    let by_name: std::collections::HashMap<&str, &ColumnInfo> = columns
        .iter()
        .map(|column| (column.name.as_str(), column))
        .collect();

    let mut specs = Vec::new();
    let mut warnings = Vec::new();
    for config in &request.columns {
        if !config.enabled {
            continue;
        }
        let Some(info) = by_name.get(config.name.as_str()) else {
            warnings.push(format!("列 {} 已不存在，已跳过", config.name));
            continue;
        };
        if info.auto_increment {
            warnings.push(format!("列 {} 为自增列，已跳过", info.name));
            continue;
        }
        if is_binary(&info.data_type) {
            return Err(format!(
                "列 {} 是二进制 / 几何类型，不能生成数据",
                info.name
            ));
        }
        if config.generator == "null" && !info.nullable {
            return Err(format!(
                "列 {} 不允许 NULL，不能使用 null 生成器",
                info.name
            ));
        }
        let mut config = config.clone();
        config.null_percent = config.null_percent.min(100);
        if config.null_percent > 0 && !info.nullable {
            warnings.push(format!("列 {} 为 NOT NULL，已忽略 NULL 比例", info.name));
            config.null_percent = 0;
        }
        if config.generator == "regex" && config.regex.trim().is_empty() {
            return Err(format!("列 {} 的正则表达式不能为空", info.name));
        }
        specs.push(ColumnSpec {
            info: (*info).clone(),
            sequence: config.sequence_start,
            is_datetime: is_datetime_type(&info.data_type),
            config,
        });
    }

    for column in columns {
        if column.auto_increment || is_binary(&column.data_type) {
            continue;
        }
        if !column.nullable && column.default_value.is_none() {
            let covered = specs.iter().any(|spec| spec.info.name == column.name);
            if !covered {
                return Err(format!(
                    "列 {} 为 NOT NULL 且没有默认值，请启用一个生成器",
                    column.name
                ));
            }
        }
    }

    if specs.is_empty() {
        return Err("没有可生成的列".to_string());
    }
    Ok((specs, warnings))
}

fn pick(chars: &[u8], rng: &mut Rng) -> char {
    chars[rng.below(chars.len() as u64) as usize] as char
}

fn random_text(rng: &mut Rng, min: usize, max: usize, charset: &str) -> String {
    let low = min.min(MAX_TEXT);
    let high = max.clamp(low, MAX_TEXT);
    let length = low + rng.below((high - low + 1) as u64) as usize;
    let bytes: &[u8] = match charset {
        "alpha" => b"abcdefghijklmnopqrstuvwxyz",
        "numeric" => b"0123456789",
        _ => b"abcdefghijklmnopqrstuvwxyz0123456789",
    };
    (0..length).map(|_| pick(bytes, rng)).collect()
}

#[derive(Debug)]
struct RegexToken {
    choices: Vec<char>,
    min: usize,
    max: usize,
}

fn parse_regex(pattern: &str) -> Result<Vec<RegexToken>, String> {
    let characters: Vec<char> = pattern.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        if matches!(characters[index], '^' | '$') {
            index += 1;
            continue;
        }
        if matches!(characters[index], '(' | ')' | '|') {
            return Err("正则仅支持字面量、字符类、\\d、\\w 与简单量词".to_string());
        }
        let choices: Vec<char> = match characters[index] {
            '\\' => {
                index += 1;
                let Some(next) = characters.get(index) else {
                    return Err("正则以反斜杠结尾".to_string());
                };
                match next {
                    'd' => ('0'..='9').collect(),
                    'w' => ('a'..='z')
                        .chain('A'..='Z')
                        .chain('0'..='9')
                        .chain(std::iter::once('_'))
                        .collect(),
                    's' => vec![' '],
                    'n' => vec!['\n'],
                    't' => vec!['\t'],
                    other if other.is_ascii_alphanumeric() => {
                        return Err(format!("不支持的正则转义 \\{other}"));
                    }
                    other => vec![*other],
                }
            }
            '[' => {
                let mut values = Vec::new();
                index += 1;
                if characters.get(index) == Some(&'^') {
                    return Err("不支持否定字符类".to_string());
                }
                while index < characters.len() && characters[index] != ']' {
                    if characters[index] == '\\'
                        && matches!(characters.get(index + 1), Some('d') | Some('w'))
                    {
                        if characters[index + 1] == 'd' {
                            values.extend('0'..='9');
                        } else {
                            values.extend('a'..='z');
                            values.extend('A'..='Z');
                            values.extend('0'..='9');
                            values.push('_');
                        }
                        index += 2;
                        continue;
                    }
                    if characters.get(index + 1) == Some(&'-')
                        && characters.get(index + 2).is_some()
                        && characters.get(index + 2) != Some(&']')
                    {
                        let start = characters[index] as u32;
                        let end = characters[index + 2] as u32;
                        if start > end {
                            return Err("字符类范围无效".to_string());
                        }
                        values.extend((start..=end).filter_map(char::from_u32));
                        index += 3;
                        continue;
                    }
                    values.push(characters[index]);
                    index += 1;
                }
                if characters.get(index) != Some(&']') {
                    return Err("字符类缺少 ]".to_string());
                }
                values
            }
            '.' => ('a'..='z').chain('0'..='9').collect(),
            other => vec![other],
        };
        index += 1;

        let (min, max) = if characters.get(index) == Some(&'{') {
            let mut cursor = index + 1;
            let start = cursor;
            while characters.get(cursor).is_some_and(|c| c.is_ascii_digit()) {
                cursor += 1;
            }
            if start == cursor {
                return Err("量词 {n} 缺少数字".to_string());
            }
            let first: usize = characters[start..cursor]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| "量词过小或过大".to_string())?;
            let max = if characters.get(cursor) == Some(&',') {
                cursor += 1;
                let start = cursor;
                while characters.get(cursor).is_some_and(|c| c.is_ascii_digit()) {
                    cursor += 1;
                }
                if start == cursor {
                    first
                } else {
                    characters[start..cursor]
                        .iter()
                        .collect::<String>()
                        .parse()
                        .map_err(|_| "量词过小或过大".to_string())?
                }
            } else {
                first
            };
            if characters.get(cursor) != Some(&'}') {
                return Err("量词缺少 }".to_string());
            }
            index = cursor + 1;
            (first, max.max(first))
        } else if characters.get(index) == Some(&'+') {
            index += 1;
            (1, 5)
        } else if characters.get(index) == Some(&'*') {
            index += 1;
            (0, 5)
        } else if characters.get(index) == Some(&'?') {
            index += 1;
            (0, 1)
        } else {
            (1, 1)
        };
        if min > 200 || max > 200 {
            return Err("正则量词不能超过 200".to_string());
        }
        if choices.is_empty() {
            return Err("正则字符类为空".to_string());
        }
        tokens.push(RegexToken { choices, min, max });
    }
    if tokens.is_empty() {
        return Err("正则表达式不能为空".to_string());
    }
    Ok(tokens)
}

fn generate_regex(pattern: &str, rng: &mut Rng) -> Result<String, String> {
    let tokens = parse_regex(pattern)?;
    let mut result = String::new();
    for token in tokens {
        let count = token.min + rng.below((token.max - token.min + 1) as u64) as usize;
        for _ in 0..count {
            result.push(token.choices[rng.below(token.choices.len() as u64) as usize]);
        }
        if result.chars().count() > MAX_TEXT {
            return Err("正则生成结果超过 10,000 字符".to_string());
        }
    }
    Ok(result)
}

fn generate_uuid(rng: &mut Rng) -> String {
    let mut bytes = [0u8; 16];
    for chunk in bytes.chunks_mut(8) {
        chunk.copy_from_slice(&rng.next_u64().to_be_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn generate_date(rng: &mut Rng, spec: &ColumnSpec) -> Result<Value, String> {
    let from = spec.config.date_from.trim();
    let to = spec.config.date_to.trim();
    // The browser supplies either `YYYY-MM-DD` or a datetime value; only the
    // date part defines the range, the time is generated randomly.
    let from_date = if from.is_empty() {
        "2000-01-01".to_string()
    } else {
        from.chars().take(10).collect()
    };
    let to_date = if to.is_empty() {
        "2030-12-31".to_string()
    } else {
        to.chars().take(10).collect()
    };
    let from = NaiveDate::parse_from_str(&from_date, "%Y-%m-%d")
        .map_err(|_| format!("列 {} 的开始日期无效", spec.info.name))?;
    let to = NaiveDate::parse_from_str(&to_date, "%Y-%m-%d")
        .map_err(|_| format!("列 {} 的结束日期无效", spec.info.name))?;
    if to < from {
        return Err(format!("列 {} 的结束日期早于开始日期", spec.info.name));
    }
    let days = (to - from).num_days().max(0);
    let date = from + Duration::days(rng.below((days + 1) as u64) as i64);
    if spec.is_datetime {
        let seconds = rng.below(86_400) as u32;
        let moment = date.and_hms_opt(0, 0, 0).unwrap() + Duration::seconds(seconds as i64);
        Ok(Value::String(
            moment.format("%Y-%m-%d %H:%M:%S").to_string(),
        ))
    } else {
        Ok(Value::String(date.format("%Y-%m-%d").to_string()))
    }
}

fn generate_value(spec: &mut ColumnSpec, rng: &mut Rng) -> Result<Value, String> {
    if spec.config.null_percent > 0 && rng.below(100) < spec.config.null_percent as u64 {
        return Ok(Value::Null);
    }
    match spec.config.generator.as_str() {
        "sequence" => {
            let current = spec.sequence;
            spec.sequence = spec.sequence.saturating_add(spec.config.sequence_step);
            Ok(Value::from(current))
        }
        "integer" => {
            let mut low = spec.config.min_number;
            let mut high = spec.config.max_number;
            if low > high {
                std::mem::swap(&mut low, &mut high);
            }
            let low = low.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64;
            let high = high.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64;
            Ok(Value::from(rng.range_i64(low, high)))
        }
        "decimal" => {
            let mut low = spec.config.min_number;
            let mut high = spec.config.max_number;
            if low > high {
                std::mem::swap(&mut low, &mut high);
            }
            let decimals = spec.config.decimals.min(6) as u32;
            let factor = 10f64.powi(decimals as i32);
            let value = (low + rng.unit() * (high - low)) * factor;
            let rounded = value.round() / factor;
            Ok(serde_json::Number::from_f64(rounded)
                .map(Value::Number)
                .unwrap_or(Value::Null))
        }
        "text" => Ok(Value::String(random_text(
            rng,
            spec.config.text_min,
            spec.config.text_max,
            &spec.config.charset,
        ))),
        "regex" => Ok(Value::String(generate_regex(&spec.config.regex, rng)?)),
        "email" => {
            let local = random_text(rng, 4, 10, "alpha");
            let domain = ["example.com", "test.local", "demo.test"][rng.below(3) as usize];
            Ok(Value::String(format!(
                "{local}{}@{domain}",
                rng.below(1000)
            )))
        }
        "name" => Ok(Value::String(next_name(rng))),
        "phone" => {
            let prefixes = ["138", "139", "150", "151", "186", "188"];
            let prefix = prefixes[rng.below(prefixes.len() as u64) as usize];
            let rest = (0..8)
                .map(|_| char::from(b'0' + rng.below(10) as u8))
                .collect::<String>();
            Ok(Value::String(format!("{prefix}{rest}")))
        }
        "company" => {
            let prefixes = ["远山", "星河", "青禾", "启明", "云帆", "北辰"];
            let suffixes = ["科技有限公司", "信息技术有限公司", "数据服务有限公司"];
            let prefix = prefixes[rng.below(prefixes.len() as u64) as usize];
            let suffix = suffixes[rng.below(suffixes.len() as u64) as usize];
            Ok(Value::String(format!("{prefix}{suffix}")))
        }
        "date" | "datetime" => generate_date(rng, spec),
        "boolean" => Ok(Value::Bool(
            rng.below(100) < spec.config.bool_percent.min(100) as u64,
        )),
        "uuid" => Ok(Value::String(generate_uuid(rng))),
        "fixed" => Ok(Value::String(spec.config.fixed_value.clone())),
        "null" => Ok(Value::Null),
        other => Err(format!(
            "列 {} 使用了不支持的生成器：{other}",
            spec.info.name
        )),
    }
}

fn next_name(rng: &mut Rng) -> String {
    let surnames = ["张", "王", "李", "赵", "陈", "林", "周", "吴", "徐", "孙"];
    let given = ["安", "宁", "晨", "悦", "然", "知", "远", "清", "禾", "星"];
    format!(
        "{}{}{}",
        surnames[rng.below(surnames.len() as u64) as usize],
        given[rng.below(given.len() as u64) as usize],
        given[rng.below(given.len() as u64) as usize]
    )
}

fn generate_rows(
    specs: &mut [ColumnSpec],
    count: usize,
    seed: u64,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(usize),
) -> Result<Vec<Vec<ColumnValue>>, String> {
    let mut rng = Rng::new(seed);
    let mut rows = Vec::with_capacity(count);
    for index in 0..count {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut values = Vec::with_capacity(specs.len());
        for spec in specs.iter_mut() {
            let value = generate_value(spec, &mut rng)
                .map_err(|error| format!("第 {} 行：{error}", index + 1))?;
            values.push(ColumnValue {
                column: spec.info.name.clone(),
                value,
            });
        }
        rows.push(values);
        if (index + 1) % 100 == 0 || index + 1 == count {
            on_progress(index + 1);
        }
    }
    Ok(rows)
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[tauri::command]
pub async fn preview_generated_data(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    request: GenerateRequest,
) -> Result<GeneratePreview, String> {
    let driver = driver_for(&app, &state, &connection_id).await?;
    let columns = driver.list_columns(&request.table).await?;
    let (mut specs, warnings) = build_specs(&columns, &request)?;
    let cancel = AtomicBool::new(false);
    let rows = generate_rows(
        &mut specs,
        request.count.min(20),
        request.seed,
        &cancel,
        |_| {},
    )?;
    let names = specs.iter().map(|spec| spec.info.name.clone()).collect();
    Ok(GeneratePreview {
        columns: names,
        rows: rows
            .into_iter()
            .map(|row| row.into_iter().map(|value| value.value).collect())
            .collect(),
        warnings,
    })
}

#[tauri::command]
pub async fn cancel_generate(
    state: State<'_, AppState>,
    operation_id: String,
) -> Result<(), String> {
    if let Some(flag) = state.generate_cancels.lock().await.get(&operation_id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn generate_table_data(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    request: GenerateRequest,
) -> Result<GenerateSummary, String> {
    ensure_writable(&app, &connection_id)?;
    let started = Instant::now();
    let driver = driver_for(&app, &state, &connection_id).await?;
    let columns = driver.list_columns(&request.table).await?;
    let (mut specs, warnings) = build_specs(&columns, &request)?;
    let operation_id = request
        .operation_id
        .clone()
        .unwrap_or_else(|| format!("generate-{}", uuid::Uuid::new_v4()));
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .generate_cancels
        .lock()
        .await
        .insert(operation_id.clone(), cancel.clone());

    let total = request.count;
    let _batch_size = request.batch_size.clamp(100, 1_000);
    let progress = |generated: usize, phase: &str, inserted: usize| {
        let _ = app.emit(
            "generate:progress",
            GenerateProgress {
                operation_id: Some(operation_id.clone()),
                phase: phase.to_string(),
                generated,
                inserted,
                total,
                message: warnings.first().cloned().unwrap_or_default(),
            },
        );
    };

    let rows = generate_rows(&mut specs, total, request.seed, &cancel, |generated| {
        progress(generated, "generating", 0)
    })?;
    if cancel.load(Ordering::Relaxed) || rows.len() < total {
        state.generate_cancels.lock().await.remove(&operation_id);
        progress(rows.len(), "cancelled", 0);
        return Ok(GenerateSummary {
            inserted: 0,
            generated: rows.len(),
            elapsed_ms: elapsed_ms(started),
            cancelled: true,
        });
    }

    progress(total, "inserting", 0);
    let changes: Vec<PendingTableChange> = rows
        .into_iter()
        .map(|values| PendingTableChange {
            kind: "insert".to_string(),
            keys: Vec::new(),
            changes: Vec::new(),
            values,
            single_row: false,
        })
        .collect();
    let outcome = driver.apply_table_changes(&request.table, &changes).await;
    state.generate_cancels.lock().await.remove(&operation_id);
    match outcome {
        Ok(inserted) => {
            progress(total, "done", inserted);
            Ok(GenerateSummary {
                inserted,
                generated: total,
                elapsed_ms: elapsed_ms(started),
                cancelled: false,
            })
        }
        Err(error) => Err(format!("生成数据写入失败（已整体回滚）: {error}")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, data_type: &str, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            data_type: data_type.to_string(),
            nullable,
            primary_key: false,
            auto_increment: false,
            default_value: None,
            comment: None,
        }
    }

    #[test]
    fn same_seed_produces_same_rows() {
        let settings = GeneratorColumn {
            name: "name".to_string(),
            generator: "text".to_string(),
            text_min: 4,
            text_max: 8,
            ..Default::default()
        };
        let columns = vec![column("name", "VARCHAR(20)", true)];
        let request = GenerateRequest {
            table: "items".to_string(),
            columns: vec![settings],
            count: 20,
            seed: 42,
            batch_size: 100,
            operation_id: None,
        };
        let (mut first, _) = build_specs(&columns, &request).expect("specs");
        let (mut second, _) = build_specs(&columns, &request).expect("specs");
        let cancel = AtomicBool::new(false);
        let rows_a = generate_rows(&mut first, 20, 42, &cancel, |_| {}).expect("rows");
        let rows_b = generate_rows(&mut second, 20, 42, &cancel, |_| {}).expect("rows");
        let values = |rows: &[Vec<ColumnValue>]| {
            rows.iter()
                .map(|row| {
                    row.iter()
                        .map(|value| value.value.clone())
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(values(&rows_a), values(&rows_b));
    }

    #[test]
    fn generates_bounded_values_and_regex() {
        let mut rng = Rng::new(7);
        for _ in 0..20 {
            let text = generate_regex("[A-Z]{2}-\\d{3}", &mut rng).expect("regex");
            assert_eq!(text.len(), 6);
            assert_eq!(&text[2..3], "-");
            let number = rng.range_i64(5, 10);
            assert!((5..=10).contains(&number));
        }
    }

    #[test]
    fn rejects_missing_not_null_generator() {
        let columns = vec![column("required", "TEXT", false)];
        let request = GenerateRequest {
            table: "items".to_string(),
            columns: vec![],
            count: 10,
            seed: 1,
            batch_size: 100,
            operation_id: None,
        };
        let error = build_specs(&columns, &request).expect_err("must reject");
        assert!(error.contains("required"));
    }
}
