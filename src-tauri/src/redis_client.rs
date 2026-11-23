use crate::models::ConnectionConfig;
use redis::aio::MultiplexedConnection;
use redis::{ConnectionAddr, ConnectionInfo, RedisConnectionInfo};
use serde::Serialize;
use std::time::Instant;

const MAX_VALUE_BYTES: usize = 100_000;
const MAX_COLLECTION_ITEMS: usize = 500;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyEntry {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisScanResult {
    pub keys: Vec<RedisKeyEntry>,
    pub scanned: usize,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisKeyDetail {
    pub key: String,
    pub key_type: String,
    pub ttl: i64,
    pub value: String,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCommandResult {
    pub output: String,
    pub elapsed_ms: u64,
    pub truncated: bool,
}

fn redis_error(context: &str, error: redis::RedisError) -> String {
    format!("{context}: {error}")
}

fn database_index(config: &ConnectionConfig) -> Result<i64, String> {
    let value = config.database.trim();
    if value.is_empty() {
        return Ok(0);
    }
    value
        .parse::<i64>()
        .map_err(|_| "Redis 数据库编号必须是整数".to_string())
        .and_then(|db| {
            if db < 0 {
                Err("Redis 数据库编号不能为负数".to_string())
            } else {
                Ok(db)
            }
        })
}

pub async fn connect(
    config: &ConnectionConfig,
    password: Option<&str>,
) -> Result<MultiplexedConnection, String> {
    let username = config.username.trim();
    let info = ConnectionInfo {
        addr: ConnectionAddr::Tcp(config.host.clone(), config.port),
        redis: RedisConnectionInfo {
            db: database_index(config)?,
            username: if username.is_empty() {
                None
            } else {
                Some(username.to_string())
            },
            password: password
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            ..Default::default()
        },
    };

    let client = redis::Client::open(info).map_err(|error| redis_error("Redis 配置无效", error))?;
    let mut connection = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|error| redis_error("Redis 连接失败", error))?;
    let pong: String = redis::cmd("PING")
        .query_async(&mut connection)
        .await
        .map_err(|error| redis_error("Redis 认证失败", error))?;
    if pong != "PONG" {
        return Err(format!("Redis 响应异常: {pong}"));
    }

    Ok(connection)
}

pub async fn test_connection(
    config: &ConnectionConfig,
    password: Option<&str>,
) -> Result<(), String> {
    connect(config, password).await.map(|_| ())
}

pub async fn ping(connection: &mut MultiplexedConnection) -> Result<String, String> {
    redis::cmd("PING")
        .query_async(connection)
        .await
        .map_err(|error| redis_error("Redis PING 失败", error))
}

pub async fn scan_keys(
    connection: &mut MultiplexedConnection,
    pattern: Option<&str>,
    limit: Option<u32>,
) -> Result<RedisScanResult, String> {
    let pattern = pattern.map(str::trim).filter(|value| !value.is_empty()).unwrap_or("*");
    let limit = limit.unwrap_or(500).clamp(1, 5000) as usize;

    let mut cursor = 0u64;
    let mut raw_keys: Vec<Vec<u8>> = Vec::new();
    let mut scanned = 0usize;
    let mut truncated = false;

    loop {
        let (next, batch): (u64, Vec<Vec<u8>>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(200)
            .query_async(connection)
            .await
            .map_err(|error| redis_error("扫描 Redis 键失败", error))?;

        scanned += batch.len();
        raw_keys.extend(batch);
        cursor = next;

        if cursor == 0 {
            break;
        }
        if raw_keys.len() >= limit {
            truncated = true;
            break;
        }
    }

    raw_keys.truncate(limit);

    let mut keys = Vec::with_capacity(raw_keys.len());
    if !raw_keys.is_empty() {
        let mut type_pipe = redis::pipe();
        let mut ttl_pipe = redis::pipe();
        for key in &raw_keys {
            type_pipe.cmd("TYPE").arg(key.as_slice());
            ttl_pipe.cmd("TTL").arg(key.as_slice());
        }

        let types: Vec<String> = type_pipe
            .query_async(connection)
            .await
            .map_err(|error| redis_error("读取 Redis 键类型失败", error))?;
        let ttls: Vec<i64> = ttl_pipe
            .query_async(connection)
            .await
            .map_err(|error| redis_error("读取 Redis TTL 失败", error))?;

        for (index, raw_key) in raw_keys.iter().enumerate() {
            keys.push(RedisKeyEntry {
                key: String::from_utf8_lossy(raw_key).to_string(),
                key_type: types
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                ttl: ttls.get(index).copied().unwrap_or(-2),
            });
        }
    }

    Ok(RedisScanResult {
        keys,
        scanned,
        truncated,
    })
}

fn join_items(items: Vec<String>, truncated: bool) -> (String, bool) {
    if items.is_empty() {
        return ("(空)".to_string(), truncated);
    }
    (items.join("\n"), truncated)
}

fn clamp_items(items: usize) -> bool {
    items > MAX_COLLECTION_ITEMS
}

pub async fn key_detail(
    connection: &mut MultiplexedConnection,
    key: &str,
) -> Result<RedisKeyDetail, String> {
    let key_bytes = key.as_bytes();
    let key_type: String = redis::cmd("TYPE")
        .arg(key_bytes)
        .query_async(connection)
        .await
        .map_err(|error| redis_error("读取 Redis 键类型失败", error))?;
    let ttl: i64 = redis::cmd("TTL")
        .arg(key_bytes)
        .query_async(connection)
        .await
        .map_err(|error| redis_error("读取 Redis TTL 失败", error))?;

    let (value, truncated) = match key_type.as_str() {
        "string" => {
            let length: i64 = redis::cmd("STRLEN")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis 字符串长度失败", error))?;
            let truncated = length as usize > MAX_VALUE_BYTES;
            let bytes: Vec<u8> = if truncated {
                redis::cmd("GETRANGE")
                    .arg(key_bytes)
                    .arg(0)
                    .arg(MAX_VALUE_BYTES as i64 - 1)
                    .query_async(connection)
                    .await
                    .map_err(|error| redis_error("读取 Redis 字符串失败", error))?
            } else {
                redis::cmd("GET")
                    .arg(key_bytes)
                    .query_async(connection)
                    .await
                    .map_err(|error| redis_error("读取 Redis 字符串失败", error))?
            };
            (String::from_utf8_lossy(&bytes).to_string(), truncated)
        }
        "hash" => {
            let length: i64 = redis::cmd("HLEN")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Hash 长度失败", error))?;
            let fields: Vec<(Vec<u8>, Vec<u8>)> = redis::cmd("HGETALL")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Hash 失败", error))?;
            let truncated = clamp_items(fields.len());
            let items = fields
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|(field, field_value)| {
                    format!(
                        "{}: {}",
                        String::from_utf8_lossy(&field),
                        String::from_utf8_lossy(&field_value)
                    )
                })
                .collect::<Vec<_>>();
            let (value, _) = join_items(items, truncated);
            (format!("共 {length} 个字段\n{value}"), truncated)
        }
        "list" => {
            let length: i64 = redis::cmd("LLEN")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis List 长度失败", error))?;
            let values: Vec<Vec<u8>> = redis::cmd("LRANGE")
                .arg(key_bytes)
                .arg(0)
                .arg(MAX_COLLECTION_ITEMS as i64 - 1)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis List 失败", error))?;
            let truncated = length as usize > values.len();
            let items = values
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    format!("{index}) {}", String::from_utf8_lossy(&value))
                })
                .collect::<Vec<_>>();
            let (value, _) = join_items(items, truncated);
            (format!("共 {length} 个元素\n{value}"), truncated)
        }
        "set" => {
            let length: i64 = redis::cmd("SCARD")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Set 长度失败", error))?;
            let mut values: Vec<Vec<u8>> = redis::cmd("SMEMBERS")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Set 失败", error))?;
            values.sort();
            let truncated = values.len() > MAX_COLLECTION_ITEMS;
            let items = values
                .into_iter()
                .take(MAX_COLLECTION_ITEMS)
                .map(|value| format!("• {}", String::from_utf8_lossy(&value)))
                .collect::<Vec<_>>();
            let (value, _) = join_items(items, truncated);
            (format!("共 {length} 个成员\n{value}"), truncated)
        }
        "zset" => {
            let length: i64 = redis::cmd("ZCARD")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis ZSet 长度失败", error))?;
            let values: Vec<(Vec<u8>, f64)> = redis::cmd("ZRANGE")
                .arg(key_bytes)
                .arg(0)
                .arg(MAX_COLLECTION_ITEMS as i64 - 1)
                .arg("WITHSCORES")
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis ZSet 失败", error))?;
            let truncated = length as usize > values.len();
            let items = values
                .into_iter()
                .map(|(member, score)| {
                    format!("{score}\t{}", String::from_utf8_lossy(&member))
                })
                .collect::<Vec<_>>();
            let (value, _) = join_items(items, truncated);
            (format!("共 {length} 个成员\n{value}"), truncated)
        }
        "stream" => {
            let length: i64 = redis::cmd("XLEN")
                .arg(key_bytes)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Stream 长度失败", error))?;
            let value: redis::Value = redis::cmd("XRANGE")
                .arg(key_bytes)
                .arg("-")
                .arg("+")
                .arg("COUNT")
                .arg(200)
                .query_async(connection)
                .await
                .map_err(|error| redis_error("读取 Redis Stream 失败", error))?;
            let output = format_value(&value);
            (
                format!("共 {length} 条记录\n{output}"),
                length > 200,
            )
        }
        "none" => ("(键不存在)".to_string(), false),
        other => (format!("暂不支持展示类型: {other}"), false),
    };

    Ok(RedisKeyDetail {
        key: key.to_string(),
        key_type,
        ttl,
        value,
        truncated,
    })
}

pub async fn delete_key(
    connection: &mut MultiplexedConnection,
    key: &str,
) -> Result<u64, String> {
    redis::cmd("DEL")
        .arg(key.as_bytes())
        .query_async(connection)
        .await
        .map_err(|error| redis_error("删除 Redis 键失败", error))
}

pub fn parse_command(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut in_argument = false;
    let mut chars = input.chars();

    while let Some(character) = chars.next() {
        match quote {
            Some(active_quote) => {
                if character == active_quote {
                    quote = None;
                } else if character == '\\' && active_quote == '"' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                } else {
                    current.push(character);
                }
            }
            None => {
                if character.is_whitespace() {
                    if in_argument {
                        args.push(std::mem::take(&mut current));
                        in_argument = false;
                    }
                } else if character == '"' || character == '\'' {
                    quote = Some(character);
                    in_argument = true;
                } else if character == '\\' {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                    in_argument = true;
                } else {
                    current.push(character);
                    in_argument = true;
                }
            }
        }
    }

    if quote.is_some() {
        return Err("命令中的引号未闭合".to_string());
    }
    if in_argument {
        args.push(current);
    }
    if args.is_empty() {
        return Err("请输入 Redis 命令".to_string());
    }

    Ok(args)
}

fn truncate_value(text: String) -> (String, bool) {
    if text.chars().count() > MAX_VALUE_BYTES {
        (
            text.chars().take(MAX_VALUE_BYTES).collect::<String>(),
            true,
        )
    } else {
        (text, false)
    }
}

fn format_value(value: &redis::Value) -> String {
    match value {
        redis::Value::Nil => "(nil)".to_string(),
        redis::Value::Int(value) => value.to_string(),
        redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        redis::Value::SimpleString(value) => value.clone(),
        redis::Value::Okay => "OK".to_string(),
        redis::Value::Double(value) => value.to_string(),
        redis::Value::Boolean(value) => value.to_string(),
        redis::Value::BigNumber(value) => value.to_string(),
        redis::Value::VerbatimString { text, .. } => text.clone(),
        redis::Value::ServerError(error) => format!("(error) {error:?}"),
        redis::Value::Array(values) => values
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .enumerate()
            .map(|(index, item)| format!("{}) {}", index + 1, format_value(item)))
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Set(values) => values
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|item| format!("• {}", format_value(item)))
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Map(pairs) => pairs
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|(key, value)| format!("{} => {}", format_value(key), format_value(value)))
            .collect::<Vec<_>>()
            .join("\n"),
        redis::Value::Attribute { data, .. } => format_value(data),
        redis::Value::Push { data, .. } => values_to_text(data),
    }
}

fn values_to_text(values: &[redis::Value]) -> String {
    values
        .iter()
        .take(MAX_COLLECTION_ITEMS)
        .enumerate()
        .map(|(index, item)| format!("{}) {}", index + 1, format_value(item)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn run_command(
    connection: &mut MultiplexedConnection,
    input: &str,
) -> Result<RedisCommandResult, String> {
    let args = parse_command(input)?;
    let mut command = redis::cmd(&args[0]);
    for argument in &args[1..] {
        command.arg(argument);
    }

    let started = Instant::now();
    let value: redis::Value = command
        .query_async(connection)
        .await
        .map_err(|error| redis_error("Redis 命令失败", error))?;
    let output = format_value(&value);
    let (output, truncated) = truncate_value(output);

    Ok(RedisCommandResult {
        output,
        elapsed_ms: started.elapsed().as_millis() as u64,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::{format_value, parse_command};

    #[test]
    fn parses_quoted_command_arguments() {
        let args = parse_command(r#"SET user:1 "张 三" 'hello world'"#).unwrap();
        assert_eq!(args, vec!["SET", "user:1", "张 三", "hello world"]);
    }

    #[test]
    fn rejects_unclosed_quotes() {
        assert!(parse_command(r#"SET key "value"#).is_err());
        assert!(parse_command("   ").is_err());
    }

    #[test]
    fn formats_scalar_and_array_values() {
        assert_eq!(format_value(&redis::Value::Okay), "OK");
        assert_eq!(format_value(&redis::Value::Nil), "(nil)");
        assert_eq!(format_value(&redis::Value::Int(42)), "42");
        let array = redis::Value::Array(vec![
            redis::Value::BulkString(b"one".to_vec()),
            redis::Value::BulkString(b"two".to_vec()),
        ]);
        assert_eq!(format_value(&array), "1) one\n2) two");
    }
}
