use super::connections::{connection_read_only, ensure_writable};
use super::mongo::{client_for, document_to_json, parse_pipeline};
use super::AppState;
use crate::models::{
    MongoConsoleConfirmation, MongoConsoleOutcome, MongoConsoleResponse,
};
use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::ReturnDocument;
use serde_json::Value;
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};

const MAX_SCRIPT_BYTES: usize = 100 * 1024;
const MAX_STATEMENTS: usize = 20;
const MAX_RESULTS: usize = 500;
const DEFAULT_FIND_LIMIT: usize = 50;
const MAX_INSERT_MANY: usize = 1_000;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) struct ParseFailure {
    pub index: usize,
    pub line: usize,
    pub message: String,
}

impl ParseFailure {
    fn new(index: usize, line: usize, message: impl Into<String>) -> Self {
        Self {
            index,
            line,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FindChain {
    sort: Option<Value>,
    skip: u64,
    limit: Option<u64>,
    count: bool,
}

#[derive(Debug, Clone)]
enum CommandKind {
    Use { database: String },
    ShowDbs,
    ShowCollections,
    ShStatus,
    Help,
    DbName,
    RawCommand {
        command: Document,
        admin: bool,
        /// Optional field path such as `.connections.current`.
        path: Vec<String>,
    },
    Call {
        collection: String,
        method: String,
        args: Vec<Value>,
        chain: FindChain,
    },
}

#[derive(Debug, Clone)]
struct ParsedCommand {
    index: usize,
    source: String,
    database: String,
    kind: CommandKind,
}

fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_collection_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| is_identifier_char(byte) || byte == b'-')
}

fn is_database_name(name: &str) -> bool {
    is_collection_name(name)
}

fn strip_comments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut index = 0usize;
    let mut quote: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            output.push(byte as char);
            if byte == b'\\' && index + 1 < bytes.len() {
                output.push(bytes[index + 1] as char);
                index += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' || byte == b'`' {
            quote = Some(byte);
            output.push(byte as char);
            index += 1;
            continue;
        }
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            index += 2;
            while index + 1 < bytes.len()
                && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
            {
                if bytes[index] == b'\n' {
                    output.push('\n');
                }
                index += 1;
            }
            index = (index + 2).min(bytes.len());
            continue;
        }
        // `#` is not mongosh syntax, but AI replies and shell snippets often
        // use it for comments; treat it as a line comment outside strings.
        if byte == b'#' {
            index += 1;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        output.push(byte as char);
        index += 1;
    }
    output
}

fn is_line_command(text: &str) -> bool {
    let cleaned = strip_comments(text);
    let text = cleaned.trim();
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("use ") || lower == "use")
        || (lower.starts_with("show ")
            && matches!(
                lower.trim_start_matches("show ").trim(),
                "dbs" | "databases" | "collections"
            ))
}

/// Splits on top-level semicolons. `use` / `show` also end at a newline so the
/// common shell style without semicolons works.
fn split_statements(script: &str) -> Vec<(usize, String)> {
    let bytes = script.as_bytes();
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut quote: Option<u8> = None;
    let mut line_comment = false;
    let mut block_comment = false;
    let mut depth = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            if byte == b'\n' {
                line_comment = false;
                if depth == 0 && is_line_command(&script[start..index]) {
                    push_statement(&mut result, start, index, script);
                    start = index + 1;
                }
            }
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'*' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if byte == b'\\' && index + 1 < bytes.len() {
                index += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' | b'`' => {
                quote = Some(byte);
                index += 1;
            }
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'/' => {
                line_comment = true;
                index += 2;
            }
            b'#' => {
                line_comment = true;
                index += 1;
            }
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'*' => {
                block_comment = true;
                index += 2;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b';' if depth == 0 => {
                push_statement(&mut result, start, index + 1, script);
                start = index + 1;
                index += 1;
            }
            b'\n' if depth == 0 && is_line_command(&script[start..index]) => {
                push_statement(&mut result, start, index, script);
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    push_statement(&mut result, start, bytes.len(), script);
    result
}

fn push_statement(
    result: &mut Vec<(usize, String)>,
    start: usize,
    end: usize,
    script: &str,
) {
    let text = &script[start..end];
    if strip_comments(text).trim().is_empty() {
        return;
    }
    result.push((start, text.to_string()));
}

fn line_at(script: &str, offset: usize) -> usize {
    script[..offset.min(script.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}

fn trim_statement(text: &str) -> String {
    let mut result = strip_comments(text).trim().to_string();
    while result.ends_with(';') {
        result.pop();
        result = result.trim_end().to_string();
    }
    result
}

/// Accepts a single- or double-quoted string literal and returns its content.
fn quoted_literal(raw: &str, label: &str, line: usize) -> Result<String, ParseFailure> {
    let raw = raw.trim();
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return Ok(raw[1..raw.len() - 1].to_string());
    }
    serde_json::from_str::<String>(raw)
        .map_err(|_| ParseFailure::new(0, line, format!("{label}必须使用引号包裹")))
}

/// Unwraps common mongosh display helpers such as `printjson(...)`,
/// `print(...)`, `console.log(...)`, `tojson(...)` and
/// `EJSON.stringify(..., null, 2)`, returning the inner command text.
fn unwrap_wrapper(text: &str, line: usize) -> Result<Option<String>, ParseFailure> {
    let trimmed = text.trim();
    let lower = trimmed.to_ascii_lowercase();
    // Tolerate shell invocations such as `mongosh "uri" --eval "db.stats()"`.
    if lower.starts_with("mongosh") || lower.starts_with("mongo ") {
        if let Some(eval_index) = lower.find("--eval") {
            let after = trimmed[eval_index + "--eval".len()..].trim_start();
            let after = after.strip_prefix('=').map(str::trim_start).unwrap_or(after);
            if after.starts_with('"') {
                let bytes = after.as_bytes();
                let mut index = 1usize;
                let mut end = None;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                        continue;
                    }
                    if bytes[index] == b'"' {
                        end = Some(index);
                        break;
                    }
                    index += 1;
                }
                if let Some(end) = end {
                    if let Ok(inner) = serde_json::from_str::<String>(&after[..=end]) {
                        return Ok(Some(inner));
                    }
                }
            } else if after.starts_with('\'') {
                if let Some(end) = after[1..].find('\'') {
                    return Ok(Some(after[1..1 + end].to_string()));
                }
            }
        }
    }
    for prefix in [
        "printjson",
        "console.log",
        "print",
        "tojson",
        "json.stringify",
        "ejson.stringify",
    ] {
        if !lower.starts_with(prefix) {
            continue;
        }
        let rest = &trimmed[prefix.len()..];
        let rest = rest.trim_start();
        if !rest.starts_with('(') {
            continue;
        }
        let close = matching_paren(rest, 0)
            .map_err(|error| ParseFailure::new(0, line, error))?;
        if !rest[close + 1..].trim().is_empty() {
            continue;
        }
        let args = split_args(&rest[1..close]);
        let inner = args
            .first()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .ok_or_else(|| ParseFailure::new(0, line, "括号内缺少命令"))?;
        return Ok(Some(inner.to_string()));
    }
    Ok(None)
}

/// Handles `db.getSiblingDB("db").<rest>` by returning the target database
/// and the remainder of the command (for example `serverStatus()`).
fn parse_sibling_scope(
    text: &str,
    line: usize,
) -> Result<Option<(String, String)>, ParseFailure> {
    let lower = text.to_ascii_lowercase();
    let prefix = "db.getsiblingdb";
    if !lower.starts_with(prefix) {
        return Ok(None);
    }
    let rest = text[prefix.len()..].trim_start();
    if !rest.starts_with('(') {
        return Err(ParseFailure::new(
            0,
            line,
            "getSiblingDB 后必须跟数据库名，例如 db.getSiblingDB(\"admin\")",
        ));
    }
    let close = matching_paren(rest, 0)
        .map_err(|error| ParseFailure::new(0, line, error))?;
    let database = quoted_literal(&rest[1..close], "数据库名", line)?;
    if !is_database_name(&database) {
        return Err(ParseFailure::new(
            0,
            line,
            "数据库名只能包含字母、数字、下划线和连字符",
        ));
    }
    let after = rest[close + 1..].trim_start();
    let remainder = after.strip_prefix('.').ok_or_else(|| {
        ParseFailure::new(
            0,
            line,
            "getSiblingDB 后必须跟方法调用，例如 db.getSiblingDB(\"admin\").serverStatus()",
        )
    })?;
    if remainder.trim().is_empty() {
        return Err(ParseFailure::new(0, line, "getSiblingDB 后缺少方法调用"));
    }
    Ok(Some((database, remainder.trim().to_string())))
}

fn parse_json_arg(text: &str, line: usize) -> Result<Value, ParseFailure> {
    serde_json::from_str::<Value>(text).map_err(|error| {
        ParseFailure::new(
            0,
            line + error.line().saturating_sub(1),
            format!(
                "参数必须是合法的 Extended JSON（键名和字符串使用双引号）：第 {} 行第 {} 列：{}",
                error.line(),
                error.column(),
                error
            ),
        )
    })
}

fn split_args(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut quote: Option<u8> = None;
    let mut depth = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' && index + 1 < bytes.len() {
                index += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' | b'`' => {
                quote = Some(byte);
                index += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b',' if depth == 0 => {
                result.push(text[start..index].trim());
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() || !text.trim().is_empty() {
        result.push(tail);
    }
    result
}

fn matching_paren(text: &str, open: usize) -> Result<usize, String> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut index = open;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(active_quote) = quote {
            if byte == b'\\' && index + 1 < bytes.len() {
                index += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' | b'`' => quote = Some(byte),
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    Err("括号未闭合".to_string())
}

/// Parses `name(args)` at the beginning of `text`; returns method, args and
/// the index immediately after the closing parenthesis.
fn parse_call(text: &str, line: usize) -> Result<(String, Vec<Value>, usize), ParseFailure> {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    if index == 0 {
        return Err(ParseFailure::new(0, line, "缺少方法名"));
    }
    let method = text[..index].to_string();
    let rest = &text[index..];
    if !rest.starts_with('(') {
        return Err(ParseFailure::new(
            0,
            line,
            format!("方法 {method} 后面必须跟括号"),
        ));
    }
    let close = matching_paren(text, index)
        .map_err(|error| ParseFailure::new(0, line, error))?;
    let args_text = &text[index + 1..close];
    let mut args = Vec::new();
    for part in split_args(args_text) {
        if part.is_empty() {
            continue;
        }
        args.push(parse_json_arg(part, line).map_err(|mut failure| {
            failure.index = 0;
            failure
        })?);
    }
    Ok((method, args, close + 1))
}

fn object_arg(value: &Value, label: &str, line: usize) -> Result<Document, ParseFailure> {
    if !value.is_object() {
        return Err(ParseFailure::new(
            0,
            line,
            format!("{label}必须是 JSON 对象"),
        ));
    }
    serde_json::from_value::<Document>(value.clone())
        .map_err(|error| ParseFailure::new(0, line, format!("{label}格式无效：{error}")))
}

fn array_arg(value: &Value, label: &str, line: usize) -> Result<Vec<Value>, ParseFailure> {
    value
        .as_array()
        .cloned()
        .ok_or_else(|| ParseFailure::new(0, line, format!("{label}必须是 JSON 数组")))
}

fn optional_object(
    args: &[Value],
    index: usize,
    label: &str,
    line: usize,
) -> Result<Option<Document>, ParseFailure> {
    match args.get(index) {
        None => Ok(None),
        Some(value) => Ok(Some(object_arg(value, label, line)?)),
    }
}

fn validate_chain(chain: &FindChain, line: usize) -> Result<(), ParseFailure> {
    if let Some(sort) = &chain.sort {
        object_arg(sort, "排序条件", line)?;
    }
    if chain.skip > 500 {
        return Err(ParseFailure::new(0, line, "skip 不能超过 500"));
    }
    if chain.limit.unwrap_or(0) > 500 {
        return Err(ParseFailure::new(0, line, "limit 不能超过 500"));
    }
    Ok(())
}

fn parse_chain(text: &str, line: usize) -> Result<(FindChain, usize), ParseFailure> {
    let mut chain = FindChain::default();
    let mut consumed = 0usize;
    let mut rest = text;
    while rest.starts_with('.') {
        let after_dot = &rest[1..];
        let (method, args, used) = parse_call(after_dot, line)?;
        match method.as_str() {
            "sort" => {
                if args.len() != 1 {
                    return Err(ParseFailure::new(0, line, ".sort() 需要一个对象参数"));
                }
                chain.sort = Some(args[0].clone());
            }
            "skip" => {
                if args.len() != 1 {
                    return Err(ParseFailure::new(0, line, ".skip() 需要一个非负整数"));
                }
                let value = args[0]
                    .as_u64()
                    .ok_or_else(|| ParseFailure::new(0, line, ".skip() 需要一个非负整数"))?;
                chain.skip = value;
            }
            "limit" => {
                if args.len() != 1 {
                    return Err(ParseFailure::new(0, line, ".limit() 需要一个非负整数"));
                }
                let value = args[0]
                    .as_u64()
                    .ok_or_else(|| ParseFailure::new(0, line, ".limit() 需要一个非负整数"))?;
                chain.limit = Some(value);
            }
            "count" => {
                if !args.is_empty() {
                    return Err(ParseFailure::new(0, line, ".count() 不需要参数"));
                }
                if chain.count {
                    return Err(ParseFailure::new(0, line, ".count() 不能重复调用"));
                }
                chain.count = true;
            }
            "pretty" => {
                if !args.is_empty() {
                    return Err(ParseFailure::new(0, line, ".pretty() 不需要参数"));
                }
            }
            other => {
                return Err(ParseFailure::new(
                    0,
                    line,
                    format!("find() 不支持链式方法 .{other}()；支持 sort / skip / limit / count / pretty"),
                ));
            }
        }
        consumed += 1 + used;
        rest = &rest[1 + used..];
        if chain.count && rest.trim_start().starts_with('.') {
            return Err(ParseFailure::new(0, line, ".count() 之后不能再链式调用"));
        }
    }
    if !rest.trim().is_empty() {
        return Err(ParseFailure::new(
            0,
            line,
            "命令末尾存在无法识别的内容",
        ));
    }
    Ok((chain, consumed))
}

fn parse_result_path(tail: &str, line: usize) -> Result<Vec<String>, ParseFailure> {
    let mut path = Vec::new();
    let mut tail = tail.trim();
    while !tail.is_empty() {
        let segment = tail.strip_prefix('.').ok_or_else(|| {
            ParseFailure::new(0, line, "只支持在结果后读取字段，例如 .connections")
        })?;
        let end = segment
            .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .unwrap_or(segment.len());
        if end == 0 {
            return Err(ParseFailure::new(0, line, "字段名不能为空"));
        }
        path.push(segment[..end].to_string());
        tail = segment[end..].trim_start();
    }
    Ok(path)
}

fn raw_command(command: Document, admin: bool, path: Vec<String>) -> CommandKind {
    CommandKind::RawCommand {
        command,
        admin,
        path,
    }
}

const DB_DIAGNOSTIC_METHODS: &[&str] = &[
    "serverStatus",
    "stats",
    "hostInfo",
    "version",
    "currentOp",
    "getProfilingStatus",
    "getProfilingLevel",
    "adminCommand",
    "runCommand",
    "getCollectionNames",
    "getCollectionInfos",
    "getUsers",
    "getRoles",
    "whoAmI",
    "getLogComponents",
    "getReplicationInfo",
    "printReplicationInfo",
    "printSecondaryReplicationInfo",
    "printSlaveReplicationInfo",
    "printShardingStatus",
    "listCommands",
    "getCmdLineOpts",
    "aggregate",
    "hello",
    "isMaster",
    "getName",
    "help",
];

fn option_fields(
    command: &mut Document,
    options: Option<&Value>,
    line: usize,
) -> Result<(), ParseFailure> {
    if let Some(Value::Object(options)) = options {
        for (key, value) in options {
            command.insert(
                key.clone(),
                super::mongo::json_to_bson(value)
                    .map_err(|error| ParseFailure::new(0, line, error))?,
            );
        }
    }
    Ok(())
}

fn parse_diagnostic_command(
    text: &str,
    line: usize,
) -> Result<Option<CommandKind>, ParseFailure> {
    if let Some(rest) = text.strip_prefix("sh.") {
        let (method, _args, used) = parse_call(rest, line)?;
        let path = parse_result_path(&rest[used..], line)?;
        return match method.as_str() {
            "status" => {
                if !path.is_empty() {
                    return Err(ParseFailure::new(
                        0,
                        line,
                        "sh.status() 不支持字段读取；结果已包含 shards / databases / balancer",
                    ));
                }
                Ok(Some(CommandKind::ShStatus))
            }
            "getBalancerState" | "isBalancerRunning" | "getBalancerWindow" => {
                Ok(Some(raw_command(doc! { "balancerStatus": 1 }, true, path)))
            }
            "help" => Ok(Some(CommandKind::Help)),
            other => Err(ParseFailure::new(
                0,
                line,
                format!(
                    "sh.{other}() 暂不支持；只读命令支持 sh.status() / sh.getBalancerState() / sh.isBalancerRunning() / sh.getBalancerWindow() / sh.help()"
                ),
            )),
        };
    }

    if let Some(rest) = text.strip_prefix("rs.") {
        let (method, _args, used) = parse_call(rest, line)?;
        let path = parse_result_path(&rest[used..], line)?;
        let command = match method.as_str() {
            "status" | "printReplicationInfo" | "printSecondaryReplicationInfo"
            | "printSlaveReplicationInfo" => doc! { "replSetGetStatus": 1 },
            "isMaster" => doc! { "isMaster": 1 },
            "hello" => doc! { "hello": 1 },
            "conf" | "config" => doc! { "replSetGetConfig": 1 },
            "help" => return Ok(Some(CommandKind::Help)),
            other => {
                return Err(ParseFailure::new(
                    0,
                    line,
                    format!(
                        "rs.{other}() 暂不支持；只读命令支持 rs.status() / rs.isMaster() / rs.hello() / rs.conf() / rs.printReplicationInfo() / rs.printSecondaryReplicationInfo() / rs.help()"
                    ),
                ))
            }
        };
        return Ok(Some(raw_command(command, true, path)));
    }

    let compact = text.split_whitespace().collect::<String>();
    if compact == "db.getMongo().getDBNames()" {
        return Ok(Some(raw_command(
            doc! { "listDatabases": 1 },
            true,
            Vec::new(),
        )));
    }

    let Some(code) = text.strip_prefix("db.") else {
        return Ok(None);
    };
    let Some(method_end) = code.find('(') else {
        return Ok(None);
    };
    let method = &code[..method_end];
    // `db.<collection>.<method>(...)` is handled by the collection parser.
    if method.contains('.') || !DB_DIAGNOSTIC_METHODS.contains(&method) {
        return Ok(None);
    }
    let (_, args, used) = parse_call(code, line)?;
    let path = parse_result_path(&code[used..], line)?;
    let first_object = || -> Result<Document, ParseFailure> {
        match args.first() {
            Some(Value::Object(object)) => {
                serde_json::from_value(Value::Object(object.clone()))
                    .map_err(|error| ParseFailure::new(0, line, format!("命令参数无效：{error}")))
            }
            Some(_) => Err(ParseFailure::new(0, line, "状态命令参数必须是 JSON 对象")),
            None => Err(ParseFailure::new(0, line, "状态命令需要一个 JSON 对象参数")),
        }
    };
    let (command, admin, path) = match method {
        "serverStatus" => (doc! { "serverStatus": 1 }, true, path),
        "stats" => (doc! { "dbStats": 1 }, false, path),
        "hostInfo" => (doc! { "hostInfo": 1 }, true, path),
        "version" => (doc! { "buildInfo": 1 }, true, path),
        "getProfilingStatus" => (doc! { "profile": -1 }, false, path),
        "getProfilingLevel" => {
            let path = if path.is_empty() {
                vec!["was".to_string()]
            } else {
                path
            };
            (doc! { "profile": -1 }, false, path)
        }
        "hello" => (doc! { "hello": 1 }, false, path),
        "isMaster" => (doc! { "isMaster": 1 }, false, path),
        "getName" => {
            if !path.is_empty() {
                return Err(ParseFailure::new(0, line, "db.getName() 不支持字段读取"));
            }
            return Ok(Some(CommandKind::DbName));
        }
        "getCollectionNames" => {
            let mut command = doc! { "listCollections": 1 };
            option_fields(&mut command, args.first(), line)?;
            (command, false, path)
        }
        "getCollectionInfos" => {
            let mut command = doc! { "listCollections": 1 };
            if let Some(Value::Object(filter)) = args.first() {
                command.insert(
                    "filter",
                    super::mongo::json_to_bson(&Value::Object(filter.clone()))
                        .map_err(|error| ParseFailure::new(0, line, error))?,
                );
            }
            (command, false, path)
        }
        "getUsers" => (doc! { "usersInfo": 1 }, false, path),
        "getRoles" => (doc! { "rolesInfo": 1 }, false, path),
        "whoAmI" => (doc! { "connectionStatus": 1 }, true, path),
        "getLogComponents" => (
            doc! { "getParameter": 1, "logComponentVerbosity": 1 },
            true,
            path,
        ),
        "getReplicationInfo" | "printReplicationInfo" | "printSecondaryReplicationInfo"
        | "printSlaveReplicationInfo" => (doc! { "replSetGetStatus": 1 }, true, path),
        "listCommands" => (doc! { "listCommands": 1 }, true, path),
        "getCmdLineOpts" => (doc! { "getCmdLineOpts": 1 }, true, path),
        "currentOp" => {
            let mut command = doc! { "currentOp": 1 };
            option_fields(&mut command, args.first(), line)?;
            (command, true, path)
        }
        "aggregate" => {
            let pipeline = args
                .first()
                .and_then(Value::as_array)
                .ok_or_else(|| ParseFailure::new(0, line, "db.aggregate() 需要一个管道数组"))?;
            let pipeline = super::mongo::json_to_bson(&Value::Array(pipeline.clone()))
                .map_err(|error| ParseFailure::new(0, line, error))?;
            (
                doc! { "aggregate": 1, "pipeline": pipeline, "cursor": doc! {} },
                false,
                path,
            )
        }
        "printShardingStatus" => return Ok(Some(CommandKind::ShStatus)),
        "help" => return Ok(Some(CommandKind::Help)),
        "adminCommand" | "runCommand" => (first_object()?, method == "adminCommand", path),
        _ => return Ok(None),
    };
    Ok(Some(raw_command(command, admin, path)))
}

fn collection_read_command(
    collection: &str,
    method: &str,
    args: &[Value],
    extra: Vec<String>,
    line: usize,
) -> Result<CommandKind, ParseFailure> {
    let coll = collection.to_string();
    let with_default = |default: &str| -> Vec<String> {
        if extra.is_empty() {
            vec![default.to_string()]
        } else {
            extra.clone()
        }
    };
    let command = match method {
        "stats" => {
            let mut command = doc! { "collStats": coll };
            option_fields(&mut command, args.first(), line)?;
            raw_command(command, false, extra)
        }
        "getIndexes" | "getIndexSpecs" => {
            let path = if extra.is_empty() {
                vec!["cursor".to_string(), "firstBatch".to_string()]
            } else {
                extra
            };
            raw_command(doc! { "listIndexes": coll }, false, path)
        }
        "dataSize" => raw_command(doc! { "collStats": coll }, false, with_default("size")),
        "storageSize" => {
            raw_command(doc! { "collStats": coll }, false, with_default("storageSize"))
        }
        "totalIndexSize" => raw_command(
            doc! { "collStats": coll },
            false,
            with_default("totalIndexSize"),
        ),
        "isCapped" => raw_command(doc! { "collStats": coll }, false, with_default("capped")),
        "count" => {
            let mut command = doc! { "count": coll };
            if let Some(Value::Object(filter)) = args.first() {
                command.insert(
                    "query",
                    super::mongo::json_to_bson(&Value::Object(filter.clone()))
                        .map_err(|error| ParseFailure::new(0, line, error))?,
                );
            }
            raw_command(command, false, with_default("n"))
        }
        _ => return Err(ParseFailure::new(0, line, "暂不支持该集合命令")),
    };
    Ok(command)
}

fn parse_call_command(text: &str, line: usize) -> Result<CommandKind, ParseFailure> {
    if let Some(command) = parse_diagnostic_command(text, line)? {
        return Ok(command);
    }
    let rest = text
        .strip_prefix("db.")
        .ok_or_else(|| ParseFailure::new(0, line, "命令必须以 db. / rs. / sh. 开头"))?;
    let (collection, after_collection) = if rest.starts_with("getCollection") {
        let after_name = &rest["getCollection".len()..];
        let after_name = after_name.trim_start();
        if !after_name.starts_with('(') {
            return Err(ParseFailure::new(
                0,
                line,
                "getCollection 后必须跟字符串参数",
            ));
        }
        let close = matching_paren(after_name, 0)
            .map_err(|error| ParseFailure::new(0, line, error))?;
        let raw = after_name[1..close].trim();
        let collection = serde_json::from_str::<String>(raw)
            .map_err(|_| ParseFailure::new(0, line, "getCollection 只接受双引号字符串"))?;
        if collection.is_empty() {
            return Err(ParseFailure::new(0, line, "集合名不能为空"));
        }
        (collection, &after_name[close + 1..])
    } else {
        let dot = rest.find('.').ok_or_else(|| {
            ParseFailure::new(0, line, "集合访问格式应为 db.<collection>.<method>(...)")
        })?;
        let collection = &rest[..dot];
        if !is_collection_name(collection) {
            return Err(ParseFailure::new(
                0,
                line,
                "集合名只能包含字母、数字、下划线和连字符；特殊名称请使用 db.getCollection(\"...\")",
            ));
        }
        (collection.to_string(), &rest[dot..])
    };
    let after_collection = after_collection.trim_start();
    if !after_collection.starts_with('.') {
        return Err(ParseFailure::new(
            0,
            line,
            "集合名后缺少方法调用，例如 db.users.find({})",
        ));
    }
    let (method, args, used) = parse_call(&after_collection[1..], line)?;
    let rest = &after_collection[1 + used..];
    if matches!(
        method.as_str(),
        "stats"
            | "getIndexes"
            | "getIndexSpecs"
            | "dataSize"
            | "storageSize"
            | "totalIndexSize"
            | "isCapped"
            | "count"
    ) {
        let extra = parse_result_path(rest, line)?;
        return collection_read_command(&collection, &method, &args, extra, line);
    }
    let chain = if method == "find" {
        let (chain, _) = parse_chain(rest, line)?;
        validate_chain(&chain, line)?;
        chain
    } else if rest.trim().is_empty() {
        FindChain::default()
    } else {
        return Err(ParseFailure::new(
            0,
            line,
            format!("{method} 不支持链式调用"),
        ));
    };
    validate_method(&method, &args, line)?;
    Ok(CommandKind::Call {
        collection,
        method,
        args,
        chain,
    })
}

fn validate_method(method: &str, args: &[Value], line: usize) -> Result<(), ParseFailure> {
    match method {
        "find" | "findOne" => {
            if args.len() > 2 {
                return Err(ParseFailure::new(0, line, format!("{method} 最多接受两个对象参数")));
            }
            for (index, value) in args.iter().enumerate() {
                object_arg(value, if index == 0 { "筛选条件" } else { "字段投影" }, line)?;
            }
        }
        "countDocuments" => {
            if args.len() > 1 {
                return Err(ParseFailure::new(0, line, "countDocuments 最多接受一个对象参数"));
            }
            if let Some(value) = args.first() {
                object_arg(value, "筛选条件", line)?;
            }
        }
        "estimatedDocumentCount" => {
            if !args.is_empty() {
                return Err(ParseFailure::new(0, line, "estimatedDocumentCount 不需要参数"));
            }
        }
        "distinct" => {
            if args.is_empty() || args.len() > 2 || !args[0].is_string() {
                return Err(ParseFailure::new(0, line, "distinct 需要字段名字符串和可选筛选对象"));
            }
            if let Some(value) = args.get(1) {
                object_arg(value, "筛选条件", line)?;
            }
        }
        "aggregate" => {
            if args.len() != 1 {
                return Err(ParseFailure::new(0, line, "aggregate 需要一个管道数组"));
            }
            let stages = array_arg(&args[0], "聚合管道", line)?;
            let pipeline = serde_json::to_string(&stages)
                .map_err(|error| ParseFailure::new(0, line, error.to_string()))?;
            parse_pipeline(&pipeline).map_err(|error| ParseFailure::new(0, line, error))?;
        }
        "insertOne" => {
            if args.len() != 1 {
                return Err(ParseFailure::new(0, line, "insertOne 需要一个文档对象"));
            }
            object_arg(&args[0], "文档", line)?;
        }
        "insertMany" => {
            if args.is_empty() || args.len() > 2 {
                return Err(ParseFailure::new(0, line, "insertMany 需要文档数组和可选选项对象"));
            }
            let docs = array_arg(&args[0], "文档数组", line)?;
            if docs.is_empty() {
                return Err(ParseFailure::new(0, line, "insertMany 的文档数组不能为空"));
            }
            if docs.len() > MAX_INSERT_MANY {
                return Err(ParseFailure::new(
                    0,
                    line,
                    format!("insertMany 单次最多 {MAX_INSERT_MANY} 个文档"),
                ));
            }
            for value in &docs {
                object_arg(value, "insertMany 文档", line)?;
            }
            optional_object(args, 1, "选项", line)?;
        }
        "updateOne" | "updateMany" | "replaceOne" | "findOneAndUpdate"
        | "findOneAndReplace" => {
            if args.len() < 2 || args.len() > 3 {
                return Err(ParseFailure::new(
                    0,
                    line,
                    format!("{method} 需要筛选对象、更新 / 文档对象和可选选项对象"),
                ));
            }
            object_arg(&args[0], "筛选条件", line)?;
            object_arg(&args[1], if method == "replaceOne" || method == "findOneAndReplace" { "替换文档" } else { "更新文档" }, line)?;
            optional_object(args, 2, "选项", line)?;
        }
        "deleteOne" | "deleteMany" | "findOneAndDelete" => {
            if args.len() != 1 {
                return Err(ParseFailure::new(0, line, format!("{method} 需要一个筛选对象")));
            }
            object_arg(&args[0], "筛选条件", line)?;
        }
        other => {
            return Err(ParseFailure::new(
                0,
                line,
                format!(
                    "暂不支持方法 db.<collection>.{other}()；支持 find / findOne / countDocuments / estimatedDocumentCount / distinct / aggregate / insert / update / replace / delete / findOneAnd* / stats / getIndexes / dataSize / storageSize / totalIndexSize / isCapped / count"
                ),
            ));
        }
    }
    Ok(())
}

fn parse_use(text: &str, line: usize) -> Result<Option<String>, ParseFailure> {
    let lower = text.to_ascii_lowercase();
    if lower == "use" {
        return Err(ParseFailure::new(0, line, "use 后需要数据库名"));
    }
    if !lower.starts_with("use ") && !lower.starts_with("use(") {
        return Ok(None);
    }
    let raw = if lower.starts_with("use(") {
        let rest = text[3..].trim_start();
        let close = matching_paren(rest, 0)
            .map_err(|error| ParseFailure::new(0, line, error))?;
        if !rest[close + 1..].trim().is_empty() {
            return Err(ParseFailure::new(0, line, "use(...) 后不能有其他内容"));
        }
        quoted_literal(&rest[1..close], "数据库名", line)?
    } else {
        text[3..].trim().to_string()
    };
    if raw.is_empty() {
        return Err(ParseFailure::new(0, line, "use 后需要数据库名"));
    }
    if !is_database_name(&raw) {
        return Err(ParseFailure::new(
            0,
            line,
            "数据库名只能包含字母、数字、下划线和连字符",
        ));
    }
    Ok(Some(raw))
}

fn parse_script(
    script: &str,
    initial_database: &str,
) -> Result<Vec<ParsedCommand>, ParseFailure> {
    if script.len() > MAX_SCRIPT_BYTES {
        return Err(ParseFailure::new(0, 0, "脚本超过 100 KB 上限"));
    }
    let mut database = if initial_database.trim().is_empty() {
        "admin".to_string()
    } else {
        initial_database.trim().to_string()
    };
    let mut commands = Vec::new();
    let mut work: Vec<(usize, String)> = split_statements(script)
        .into_iter()
        .rev()
        .collect();
    while let Some((start, raw)) = work.pop() {
        let line = line_at(script, start);
        let mut text = trim_statement(&raw);
        if text.is_empty() {
            continue;
        }
        // Accept mongosh display helpers such as printjson(db.stats()).
        for _ in 0..4 {
            match unwrap_wrapper(&text, line)? {
                Some(inner) => text = inner,
                None => break,
            }
        }
        // AI replies sometimes wrap several calls in a JS array literal.
        if text.starts_with('[') && text.ends_with(']') {
            let parts = split_args(&text[1..text.len() - 1]);
            if parts.is_empty() {
                return Err(ParseFailure::new(0, line, "数组中没有可执行的命令"));
            }
            for part in parts.into_iter().rev() {
                work.push((start, part.to_string()));
            }
            continue;
        }
        if commands.len() >= MAX_STATEMENTS {
            return Err(ParseFailure::new(
                commands.len() + 1,
                line,
                format!("单次最多执行 {MAX_STATEMENTS} 条命令"),
            ));
        }
        let index = commands.len() + 1;
        let use_target = parse_use(&text, line).map_err(|mut failure| {
            failure.index = index;
            failure
        })?;
        if let Some(target) = use_target {
            let command = ParsedCommand {
                index,
                source: text.clone(),
                database: target.clone(),
                kind: CommandKind::Use {
                    database: target.clone(),
                },
            };
            database = target;
            commands.push(command);
            continue;
        }
        let lower = text.to_ascii_lowercase();
        if lower == "help" {
            commands.push(ParsedCommand {
                index,
                source: text,
                database: database.clone(),
                kind: CommandKind::Help,
            });
            continue;
        }
        if lower == "show dbs" || lower == "show databases" {
            commands.push(ParsedCommand {
                index,
                source: text,
                database: database.clone(),
                kind: CommandKind::ShowDbs,
            });
            continue;
        }
        if lower == "show collections" {
            commands.push(ParsedCommand {
                index,
                source: text,
                database: database.clone(),
                kind: CommandKind::ShowCollections,
            });
            continue;
        }
        let show_command = match lower.as_str() {
            "show users" => Some(doc! { "usersInfo": 1 }),
            "show roles" => Some(doc! { "rolesInfo": 1 }),
            "show profile" => Some(doc! { "profile": -1 }),
            _ => None,
        };
        if let Some(command) = show_command {
            commands.push(ParsedCommand {
                index,
                source: text,
                database: database.clone(),
                kind: raw_command(command, false, Vec::new()),
            });
            continue;
        }
        if lower.starts_with("show ") {
            return Err(ParseFailure::new(
                index,
                line,
                "支持 show dbs / show databases / show collections / show users / show roles / show profile",
            ));
        }
        if let Some((target, remainder)) = parse_sibling_scope(&text, line)? {
            let scoped = format!("db.{remainder}");
            let kind = parse_call_command(&scoped, line).map_err(|mut failure| {
                failure.index = index;
                failure
            })?;
            commands.push(ParsedCommand {
                index,
                source: text,
                database: target,
                kind,
            });
            continue;
        }
        if !text.starts_with("db.")
            && !text.starts_with("rs.")
            && !text.starts_with("sh.")
        {
            return Err(ParseFailure::new(
                index,
                line,
                "无法识别这条命令；输入 help 可查看完整命令清单，支持 use <db> / show ...、db.<collection>.<method>(...)、db.serverStatus() 等状态命令、rs.status() 和 sh.status()，也支持 printjson(...) 和 db.getSiblingDB(\"db\")... 包裹",
            ));
        }
        let kind = parse_call_command(&text, line).map_err(|mut failure| {
            failure.index = index;
            failure
        })?;
        commands.push(ParsedCommand {
            index,
            source: text,
            database: database.clone(),
            kind,
        });
    }
    Ok(commands)
}

fn is_write_method(method: &str) -> bool {
    matches!(
        method,
        "insertOne"
            | "insertMany"
            | "updateOne"
            | "updateMany"
            | "replaceOne"
            | "deleteOne"
            | "deleteMany"
            | "findOneAndUpdate"
            | "findOneAndReplace"
            | "findOneAndDelete"
    )
}

fn call_parts(command: &ParsedCommand) -> Option<(&str, &str, &[Value])> {
    match &command.kind {
        CommandKind::Call {
            collection,
            method,
            args,
            ..
        } => Some((collection, method, args)),
        _ => None,
    }
}

fn empty_filter(args: &[Value]) -> bool {
    args.first()
        .and_then(Value::as_object)
        .map(|object| object.is_empty())
        .unwrap_or(false)
}

fn confirmation_reason(command: &ParsedCommand) -> Option<String> {
    let (_, method, args) = call_parts(command)?;
    if method == "insertMany" {
        let count = args.first().and_then(Value::as_array).map(Vec::len).unwrap_or(0);
        if count > 100 {
            return Some(format!("将插入 {count} 个文档，超过 100 个"));
        }
    }
    if matches!(
        method,
        "updateMany"
            | "deleteMany"
            | "replaceOne"
            | "deleteOne"
            | "findOneAndUpdate"
            | "findOneAndReplace"
            | "findOneAndDelete"
    ) && empty_filter(args)
    {
        return Some("筛选条件为空，可能影响整个集合".to_string());
    }
    None
}

async fn estimate_documents(
    client: &mongodb::Client,
    database: &str,
    collection: &str,
) -> Option<u64> {
    let handle = client.database(database).collection::<Document>(collection);
    tokio::time::timeout(Duration::from_secs(3), handle.estimated_document_count())
        .await
        .ok()
        .and_then(Result::ok)
}

fn bson_to_json(value: &Bson) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn summary_document(document: Document) -> Result<Vec<Value>, String> {
    Ok(vec![document_to_json(&document)?])
}

fn help_document() -> Document {
    let value = serde_json::json!({
        "用法": "参数使用 Extended JSON：键名和字符串用双引号；多条命令用换行或分号分隔",
        "数据库": [
            "use <db>", "show dbs", "show databases", "show collections",
            "show users", "show roles", "show profile",
            "db.getName()", "db.getMongo().getDBNames()"
        ],
        "查询": [
            "db.<collection>.find({...})", "db.<collection>.findOne({...})",
            "db.<collection>.aggregate([...])", "db.<collection>.countDocuments({...})",
            "db.<collection>.estimatedDocumentCount()",
            "db.<collection>.distinct(\"field\", {...})", "db.<collection>.count({...})",
            "db.aggregate([...])"
        ],
        "集合信息": [
            "db.getCollectionNames()", "db.getCollectionInfos({...})",
            "db.<collection>.stats()", "db.<collection>.getIndexes()",
            "db.<collection>.dataSize()", "db.<collection>.storageSize()",
            "db.<collection>.totalIndexSize()", "db.<collection>.isCapped()"
        ],
        "状态诊断": [
            "db.serverStatus()", "db.stats()", "db.hostInfo()", "db.version()",
            "db.currentOp({...})", "db.getProfilingStatus()", "db.getProfilingLevel()",
            "db.whoAmI()", "db.listCommands()", "db.getCmdLineOpts()", "db.getLogComponents()"
        ],
        "用户权限": ["db.getUsers()", "db.getRoles()"],
        "复制集": [
            "rs.status()", "rs.isMaster()", "rs.hello()", "rs.conf()",
            "rs.printReplicationInfo()", "rs.printSecondaryReplicationInfo()"
        ],
        "分片": [
            "sh.status()", "sh.getBalancerState()", "sh.isBalancerRunning()",
            "sh.getBalancerWindow()"
        ],
        "字段读取": [
            "db.serverStatus().connections.current", "db.stats().collections",
            "db.<collection>.stats().size"
        ],
        "其他写法": [
            "db.adminCommand({...})", "db.runCommand({...})", "printjson(...)",
            "console.log(...)", "EJSON.stringify(...)", "db.getSiblingDB(\"db\")...",
            "mongosh --eval \"...\""
        ]
    });
    match super::mongo::json_to_bson(&value) {
        Ok(Bson::Document(document)) => document,
        _ => Document::new(),
    }
}

fn warning_for(source: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if source.contains("$where") || source.contains("$function") {
        warnings.push("该命令会在服务器执行 JavaScript；请确认查询内容可信。".to_string());
    }
    warnings
}

fn clean_error(error: impl std::fmt::Display) -> String {
    error.to_string().replace('\n', " ")
}

fn option_bool(options: Option<&Document>, key: &str, default: bool) -> Result<bool, String> {
    match options.and_then(|document| document.get(key)) {
        None => Ok(default),
        Some(Bson::Boolean(value)) => Ok(*value),
        Some(_) => Err(format!("选项 {key} 必须是布尔值")),
    }
}

fn option_string<'a>(
    options: Option<&'a Document>,
    key: &str,
) -> Result<Option<&'a str>, String> {
    match options.and_then(|document| document.get(key)) {
        None => Ok(None),
        Some(Bson::String(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(format!("选项 {key} 必须是字符串")),
    }
}

async fn execute_command(
    app: &AppHandle,
    _state: &AppState,
    client: &mongodb::Client,
    connection_id: &str,
    command: &ParsedCommand,
) -> Result<MongoConsoleOutcome, String> {
    let started = Instant::now();
    let mut outcome = MongoConsoleOutcome {
        index: command.index,
        statement: truncate(&command.source, 200),
        kind: "empty".to_string(),
        database: command.database.clone(),
        collection: None,
        documents: Vec::new(),
        values: Vec::new(),
        elapsed_ms: 0,
        truncated: false,
        error: None,
        message: None,
        warnings: warning_for(&command.source),
    };
    match &command.kind {
        CommandKind::Use { database } => {
            outcome.database = database.clone();
            outcome.message = Some(format!("已切换到数据库 {database}"));
        }
        CommandKind::ShowDbs => {
            let mut names = client
                .list_database_names()
                .await
                .map_err(|error| format!("读取数据库失败：{}", clean_error(error)))?;
            names.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
            outcome.kind = "documents".to_string();
            outcome.documents = names
                .into_iter()
                .map(|name| Value::Object(serde_json::Map::from_iter([(
                    "name".to_string(),
                    Value::String(name),
                )])))
                .collect();
        }
        CommandKind::ShowCollections => {
            let collections = client
                .database(&command.database)
                .list_collections()
                .await
                .map_err(|error| format!("读取集合失败：{}", clean_error(error)))?
                .try_collect::<Vec<_>>()
                .await
                .map_err(|error| format!("读取集合失败：{}", clean_error(error)))?;
            outcome.kind = "documents".to_string();
            outcome.documents = collections
                .into_iter()
                .map(|item| {
                    let mut object = serde_json::Map::new();
                    object.insert("name".to_string(), Value::String(item.name));
                    object.insert(
                        "type".to_string(),
                        Value::String(format!("{:?}", item.collection_type).to_lowercase()),
                    );
                    Value::Object(object)
                })
                .collect();
        }
        CommandKind::Help => {
            outcome.kind = "document".to_string();
            outcome.documents = summary_document(help_document())?;
        }
        CommandKind::DbName => {
            outcome.kind = "document".to_string();
            outcome.documents = summary_document(doc! { "db": command.database.clone() })?;
        }
        CommandKind::ShStatus => {
            let shards = client
                .database("admin")
                .run_command(doc! { "listShards": 1 })
                .await
                .map_err(|error| {
                    format!(
                        "sh.status() 需要连接到 mongos（分片集群路由）：{}",
                        clean_error(error)
                    )
                })?;
            let databases = client
                .database("admin")
                .run_command(doc! { "listDatabases": 1 })
                .await
                .ok();
            let balancer = client
                .database("admin")
                .run_command(doc! { "balancerStatus": 1 })
                .await
                .ok();
            let mut summary = Document::new();
            if let Some(Bson::Array(items)) = shards.get("shards") {
                summary.insert("shards", Bson::Array(items.clone()));
            }
            if let Some(databases) = databases {
                if let Some(Bson::Array(items)) = databases.get("databases") {
                    summary.insert("databases", Bson::Array(items.clone()));
                }
            }
            if let Some(balancer) = balancer {
                summary.insert("balancer", Bson::Document(balancer));
            }
            outcome.kind = "document".to_string();
            outcome.documents = summary_document(summary)?;
        }
        CommandKind::RawCommand {
            command: raw,
            admin,
            path,
        } => {
            let database = if *admin {
                "admin"
            } else {
                command.database.as_str()
            };
            let result = client
                .database(database)
                .run_command(raw.clone())
                .await
                .map_err(|error| format!("执行状态命令失败：{}", clean_error(error)))?;
            outcome.kind = "document".to_string();
            if path.is_empty() {
                outcome.documents = summary_document(result)?;
            } else {
                let mut current = Bson::Document(result);
                for segment in path {
                    let Bson::Document(document) = current else {
                        return Err(format!("字段路径 .{} 之前的值不是文档", segment));
                    };
                    current = document
                        .get(segment)
                        .cloned()
                        .ok_or_else(|| format!("结果中不存在字段 .{}", path.join(".")))?;
                }
                let key = path.last().cloned().unwrap_or_else(|| "value".to_string());
                let mut summary = Document::new();
                summary.insert(key, current);
                outcome.documents = summary_document(summary)?;
            }
        }
        CommandKind::Call {
            collection,
            method,
            args,
            chain,
        } => {
            outcome.collection = Some(collection.clone());
            let handle = client
                .database(&command.database)
                .collection::<Document>(collection);
            let filter = match args.first() {
                Some(value) if value.is_object() => object_from_value(value, "筛选条件")?,
                _ => Document::new(),
            };
            match method.as_str() {
                "find" => {
                    if chain.count {
                        let count = handle
                            .count_documents(filter)
                            .await
                            .map_err(|error| format!("计数失败：{}", clean_error(error)))?;
                        outcome.documents = summary_document(doc! { "count": count as i64 })?;
                        outcome.kind = "scalar".to_string();
                    } else {
                        let projection = args
                            .get(1)
                            .map(|value| object_from_value(value, "字段投影"))
                            .transpose()?
                            .unwrap_or_default();
                        let sort = chain
                            .sort
                            .as_ref()
                            .map(|value| object_from_value(value, "排序条件"))
                            .transpose()?
                            .unwrap_or_else(|| doc! { "_id": 1 });
                        let limit = match chain.limit {
                            None => DEFAULT_FIND_LIMIT as u64,
                            Some(0) => MAX_RESULTS as u64,
                            Some(value) => value.min(MAX_RESULTS as u64),
                        };
                        let mut find = handle
                            .find(filter)
                            .sort(sort)
                            .skip(chain.skip)
                            .limit(limit as i64);
                        if !projection.is_empty() {
                            find = find.projection(projection);
                        }
                        let mut cursor = find
                            .await
                            .map_err(|error| format!("查询失败：{}", clean_error(error)))?;
                        while let Some(document) = cursor
                            .try_next()
                            .await
                            .map_err(|error| format!("读取结果失败：{}", clean_error(error)))?
                        {
                            outcome.documents.push(document_to_json(&document)?);
                        }
                        outcome.truncated = outcome.documents.len() as u64 >= limit;
                        outcome.kind = "documents".to_string();
                    }
                }
                "findOne" => {
                    let projection = args
                        .get(1)
                        .map(|value| object_from_value(value, "字段投影"))
                        .transpose()?
                        .unwrap_or_default();
                    let mut find = handle.find_one(filter);
                    if !projection.is_empty() {
                        find = find.projection(projection);
                    }
                    let document = find
                        .await
                        .map_err(|error| format!("查询失败：{}", clean_error(error)))?;
                    if let Some(document) = document {
                        outcome.documents.push(document_to_json(&document)?);
                        outcome.kind = "document".to_string();
                    } else {
                        outcome.kind = "empty".to_string();
                        outcome.message = Some("未找到文档".to_string());
                    }
                }
                "countDocuments" => {
                    let count = handle
                        .count_documents(filter)
                        .await
                        .map_err(|error| format!("计数失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! { "count": count as i64 })?;
                    outcome.kind = "scalar".to_string();
                }
                "estimatedDocumentCount" => {
                    let count = handle
                        .estimated_document_count()
                        .await
                        .map_err(|error| format!("估算计数失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! { "count": count as i64 })?;
                    outcome.kind = "scalar".to_string();
                    outcome.message = Some("估算值".to_string());
                }
                "distinct" => {
                    let field = args
                        .first()
                        .and_then(Value::as_str)
                        .ok_or_else(|| "distinct 需要字段名".to_string())?;
                    let filter = args
                        .get(1)
                        .map(|value| object_from_value(value, "筛选条件"))
                        .transpose()?
                        .unwrap_or_default();
                    let values = handle
                        .distinct(field, filter)
                        .await
                        .map_err(|error| format!("distinct 失败：{}", clean_error(error)))?;
                    let total = values.len();
                    outcome.values = values
                        .into_iter()
                        .take(MAX_RESULTS)
                        .map(|value| bson_to_json(&value))
                        .collect();
                    outcome.truncated = total > MAX_RESULTS;
                    outcome.kind = "values".to_string();
                }
                "aggregate" => {
                    let stages = args
                        .first()
                        .and_then(Value::as_array)
                        .ok_or_else(|| "aggregate 需要数组".to_string())?;
                    let pipeline = parse_pipeline(
                        &serde_json::to_string(stages)
                            .map_err(|error| format!("聚合管道格式错误：{error}"))?,
                    )?;
                    let mut cursor = handle
                        .aggregate(pipeline)
                        .await
                        .map_err(|error| format!("聚合失败：{}", clean_error(error)))?;
                    while let Some(document) = cursor
                        .try_next()
                        .await
                        .map_err(|error| format!("读取聚合结果失败：{}", clean_error(error)))?
                    {
                        outcome.documents.push(document_to_json(&document)?);
                        if outcome.documents.len() >= MAX_RESULTS {
                            break;
                        }
                    }
                    outcome.truncated = outcome.documents.len() >= MAX_RESULTS;
                    outcome.kind = "documents".to_string();
                }
                "insertOne" => {
                    ensure_writable(app, connection_id)?;
                    let document = object_from_value(
                        args.first().ok_or_else(|| "insertOne 需要文档".to_string())?,
                        "文档",
                    )?;
                    let result = handle
                        .insert_one(document)
                        .await
                        .map_err(|error| format!("插入失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! {
                        "acknowledged": true,
                        "insertedId": result.inserted_id,
                    })?;
                    outcome.kind = "write".to_string();
                    outcome.message = Some("已插入 1 个文档".to_string());
                }
                "insertMany" => {
                    ensure_writable(app, connection_id)?;
                    let values = args
                        .first()
                        .and_then(Value::as_array)
                        .ok_or_else(|| "insertMany 需要文档数组".to_string())?;
                    if values.is_empty() || values.len() > MAX_INSERT_MANY {
                        return Err(format!("insertMany 单次需要 1–{MAX_INSERT_MANY} 个文档"));
                    }
                    let documents = values
                        .iter()
                        .map(|value| object_from_value(value, "文档"))
                        .collect::<Result<Vec<_>, _>>()?;
                    let options = args
                        .get(1)
                        .map(|value| object_from_value(value, "选项"))
                        .transpose()?;
                    let ordered = option_bool(options.as_ref(), "ordered", true)?;
                    let result = handle
                        .insert_many(documents)
                        .ordered(ordered)
                        .await
                        .map_err(|error| format!("批量插入失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! {
                        "acknowledged": true,
                        "insertedCount": result.inserted_ids.len() as i64,
                    })?;
                    outcome.kind = "write".to_string();
                    outcome.message = Some(format!("已插入 {} 个文档", result.inserted_ids.len()));
                }
                "updateOne" | "updateMany" => {
                    ensure_writable(app, connection_id)?;
                    let update = object_from_value(
                        args.get(1)
                            .ok_or_else(|| "update 需要更新文档".to_string())?,
                        "更新文档",
                    )?;
                    let options = args
                        .get(2)
                        .map(|value| object_from_value(value, "选项"))
                        .transpose()?;
                    let upsert = option_bool(options.as_ref(), "upsert", false)?;
                    let result = if method == "updateOne" {
                        handle.update_one(filter, update).upsert(upsert).await
                    } else {
                        handle.update_many(filter, update).upsert(upsert).await
                    }
                    .map_err(|error| format!("更新失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! {
                        "acknowledged": true,
                        "matchedCount": result.matched_count as i64,
                        "modifiedCount": result.modified_count as i64,
                        "upsertedId": result.upserted_id,
                    })?;
                    outcome.kind = "write".to_string();
                    outcome.message = Some(format!(
                        "匹配 {} 个，修改 {} 个",
                        result.matched_count, result.modified_count
                    ));
                }
                "replaceOne" => {
                    ensure_writable(app, connection_id)?;
                    let replacement = object_from_value(
                        args.get(1)
                            .ok_or_else(|| "replaceOne 需要替换文档".to_string())?,
                        "替换文档",
                    )?;
                    let options = args
                        .get(2)
                        .map(|value| object_from_value(value, "选项"))
                        .transpose()?;
                    let upsert = option_bool(options.as_ref(), "upsert", false)?;
                    let result = handle
                        .replace_one(filter, replacement)
                        .upsert(upsert)
                        .await
                        .map_err(|error| format!("替换失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! {
                        "acknowledged": true,
                        "matchedCount": result.matched_count as i64,
                        "modifiedCount": result.modified_count as i64,
                        "upsertedId": result.upserted_id,
                    })?;
                    outcome.kind = "write".to_string();
                    outcome.message = Some(format!(
                        "匹配 {} 个，修改 {} 个",
                        result.matched_count, result.modified_count
                    ));
                }
                "deleteOne" | "deleteMany" => {
                    ensure_writable(app, connection_id)?;
                    let result = if method == "deleteOne" {
                        handle.delete_one(filter).await
                    } else {
                        handle.delete_many(filter).await
                    }
                    .map_err(|error| format!("删除失败：{}", clean_error(error)))?;
                    outcome.documents = summary_document(doc! {
                        "acknowledged": true,
                        "deletedCount": result.deleted_count as i64,
                    })?;
                    outcome.kind = "write".to_string();
                    outcome.message = Some(format!("已删除 {} 个文档", result.deleted_count));
                }
                "findOneAndUpdate" | "findOneAndReplace" => {
                    ensure_writable(app, connection_id)?;
                    let replacement = object_from_value(
                        args.get(1)
                            .ok_or_else(|| "findOneAnd* 需要第二个文档".to_string())?,
                        "文档",
                    )?;
                    let options = args
                        .get(2)
                        .map(|value| object_from_value(value, "选项"))
                        .transpose()?;
                    let return_document = match option_string(options.as_ref(), "returnDocument")? {
                        Some("after") => ReturnDocument::After,
                        Some("before") | None => ReturnDocument::Before,
                        Some(_) => return Err("returnDocument 只能是 before 或 after".to_string()),
                    };
                    let upsert = option_bool(options.as_ref(), "upsert", false)?;
                    let result = if method == "findOneAndUpdate" {
                        handle
                            .find_one_and_update(filter, replacement)
                            .return_document(return_document)
                            .upsert(upsert)
                            .await
                    } else {
                        handle
                            .find_one_and_replace(filter, replacement)
                            .return_document(return_document)
                            .upsert(upsert)
                            .await
                    }
                    .map_err(|error| format!("findOneAnd* 失败：{}", clean_error(error)))?;
                    if let Some(document) = result {
                        outcome.documents.push(document_to_json(&document)?);
                        outcome.kind = "document".to_string();
                    } else {
                        outcome.kind = "empty".to_string();
                        outcome.message = Some("未找到文档".to_string());
                    }
                }
                "findOneAndDelete" => {
                    ensure_writable(app, connection_id)?;
                    let result = handle
                        .find_one_and_delete(filter)
                        .await
                        .map_err(|error| format!("findOneAndDelete 失败：{}", clean_error(error)))?;
                    if let Some(document) = result {
                        outcome.documents.push(document_to_json(&document)?);
                        outcome.kind = "document".to_string();
                    } else {
                        outcome.kind = "empty".to_string();
                        outcome.message = Some("未找到文档".to_string());
                    }
                }
                other => return Err(format!("暂不支持方法 {other}")),
            }
        }
    }
    outcome.elapsed_ms = started.elapsed().as_millis() as u64;
    Ok(outcome)
}

fn object_from_value(value: &Value, label: &str) -> Result<Document, String> {
    if !value.is_object() {
        return Err(format!("{label}必须是 JSON 对象"));
    }
    serde_json::from_value::<Document>(value.clone())
        .map_err(|error| format!("{label}格式无效：{error}"))
}

fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_string()
    } else {
        format!("{}…", value.chars().take(limit.saturating_sub(1)).collect::<String>())
    }
}

#[tauri::command]
pub async fn mongo_console_execute(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    script: String,
    confirmed: bool,
) -> Result<MongoConsoleResponse, String> {
    let commands = match parse_script(&script, &database) {
        Ok(commands) => commands,
        Err(failure) => {
            return Ok(MongoConsoleResponse {
                outcomes: Vec::new(),
                confirmations: Vec::new(),
                database,
                refresh_collections: false,
                error: Some(format!(
                    "第 {} 行（第 {} 条）：{}",
                    failure.line.max(1),
                    failure.index.max(1),
                    failure.message
                )),
                error_index: Some(failure.index),
            });
        }
    };
    if commands.is_empty() {
        return Ok(MongoConsoleResponse {
            outcomes: Vec::new(),
            confirmations: Vec::new(),
            database,
            refresh_collections: false,
            error: Some("没有可执行的命令".to_string()),
            error_index: None,
        });
    }
    let read_only = connection_read_only(&app, &connection_id)?;
    if read_only {
        if let Some(command) = commands.iter().find(|command| {
            call_parts(command)
                .map(|(_, method, _)| is_write_method(method))
                .unwrap_or(false)
        }) {
            return Ok(MongoConsoleResponse {
                outcomes: Vec::new(),
                confirmations: Vec::new(),
                database,
                refresh_collections: false,
                error: Some(format!(
                    "第 {} 条：当前连接为只读模式，写命令已被拒绝；读命令仍可执行",
                    command.index
                )),
                error_index: Some(command.index),
            });
        }
    }
    let client = client_for(&app, &state, &connection_id).await?;
    let mut confirmations = Vec::new();
    for command in &commands {
        let Some(reason) = confirmation_reason(command) else {
            continue;
        };
        let (collection, method, _) = call_parts(command).unwrap_or(("", "", &[]));
        let estimated_count = estimate_documents(&client, &command.database, collection).await;
        confirmations.push(MongoConsoleConfirmation {
            index: command.index,
            database: command.database.clone(),
            collection: collection.to_string(),
            method: method.to_string(),
            reason,
            estimated_count,
            preview: truncate(&command.source, 240),
        });
    }
    let mut current_database = database;
    if !confirmations.is_empty() && !confirmed {
        return Ok(MongoConsoleResponse {
            outcomes: Vec::new(),
            confirmations,
            database: current_database,
            refresh_collections: false,
            error: None,
            error_index: None,
        });
    }
    let mut outcomes = Vec::new();
    let mut refresh_collections = false;
    for command in commands {
        if let CommandKind::Use { database } = &command.kind {
            current_database = database.clone();
        }
        let first_write = call_parts(&command)
            .map(|(_, method, _)| is_write_method(method))
            .unwrap_or(false);
        let result = tokio::time::timeout(
            COMMAND_TIMEOUT,
            execute_command(&app, &state, &client, &connection_id, &command),
        )
        .await;
        match result {
            Ok(Ok(outcome)) => {
                if first_write {
                    refresh_collections = true;
                }
                outcomes.push(outcome);
            }
            Ok(Err(error)) => {
                let mut outcome = MongoConsoleOutcome {
                    index: command.index,
                    statement: truncate(&command.source, 200),
                    kind: "empty".to_string(),
                    database: command.database.clone(),
                    collection: None,
                    documents: Vec::new(),
                    values: Vec::new(),
                    elapsed_ms: 0,
                    truncated: false,
                    error: Some(error.clone()),
                    message: None,
                    warnings: warning_for(&command.source),
                };
                if let Some((collection, _, _)) = call_parts(&command) {
                    outcome.collection = Some(collection.to_string());
                }
                outcomes.push(outcome);
                return Ok(MongoConsoleResponse {
                    outcomes,
                    confirmations: Vec::new(),
                    database: current_database,
                    refresh_collections,
                    error: Some(error),
                    error_index: Some(command.index),
                });
            }
            Err(_) => {
                let message = if first_write {
                    "命令超时（30 秒）；写入可能已经在服务器生效，请先查询确认，不要自动重跑"
                        .to_string()
                } else {
                    "命令超时（30 秒）".to_string()
                };
                outcomes.push(MongoConsoleOutcome {
                    index: command.index,
                    statement: truncate(&command.source, 200),
                    kind: "empty".to_string(),
                    database: command.database.clone(),
                    collection: None,
                    documents: Vec::new(),
                    values: Vec::new(),
                    elapsed_ms: COMMAND_TIMEOUT.as_millis() as u64,
                    truncated: false,
                    error: Some(message.clone()),
                    message: None,
                    warnings: warning_for(&command.source),
                });
                return Ok(MongoConsoleResponse {
                    outcomes,
                    confirmations: Vec::new(),
                    database: current_database,
                    refresh_collections,
                    error: Some(message),
                    error_index: Some(command.index),
                });
            }
        }
    }
    Ok(MongoConsoleResponse {
        outcomes,
        confirmations: Vec::new(),
        database: current_database,
        refresh_collections,
        error: None,
        error_index: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_respects_strings_comments_and_line_commands() {
        let script = "use app;\n// comment; still\n db.users.find({\"name\":\"a;b\"});\nshow collections";
        let statements = split_statements(script);
        assert_eq!(statements.len(), 3);
        assert!(statements[0].1.contains("use app"));
        assert!(statements[1].1.contains("find"));
        assert!(statements[2].1.to_ascii_lowercase().contains("show collections"));
    }

    #[test]
    fn parses_mongodb_status_commands() {
        let commands = parse_script(
            "db.serverStatus(); db.stats(); db.currentOp({\"active\":true}); db.adminCommand({\"ping\":1})",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 4);
        assert!(matches!(commands[0].kind, CommandKind::RawCommand { admin: true, .. }));
        assert!(matches!(commands[1].kind, CommandKind::RawCommand { admin: false, .. }));
        assert!(matches!(commands[2].kind, CommandKind::RawCommand { admin: true, .. }));
        assert!(matches!(commands[3].kind, CommandKind::RawCommand { admin: true, .. }));
    }

    #[test]
    fn parses_mongosh_wrappers_and_hash_comments() {
        let commands = parse_script(
            "# 查看数据库状态\nprintjson(EJSON.stringify(db.serverStatus(), null, 2))",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            commands[0].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
    }

    #[test]
    fn parses_extended_read_only_catalog() {
        let commands = parse_script("help", "app").unwrap();
        assert!(matches!(commands[0].kind, CommandKind::Help));

        let commands = parse_script(
            "show users; show roles; show profile;\
             db.getUsers(); db.getRoles(); db.whoAmI(); db.listCommands();\
             db.getCmdLineOpts(); db.getLogComponents(); db.getName();\
             db.hello(); db.isMaster(); db.getCollectionInfos({\"nameOnly\":true});\
             db.aggregate([{\"$currentOp\":{}}])",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 14);
        for (position, command) in commands.iter().enumerate() {
            if position == 9 {
                assert!(matches!(command.kind, CommandKind::DbName));
            } else {
                assert!(matches!(command.kind, CommandKind::RawCommand { .. }));
            }
        }

        let commands = parse_script(
            "db.users.stats(); db.users.getIndexes(); db.users.dataSize();\
             db.users.isCapped(); db.users.count({\"active\":true});\
             db.users.stats().size",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 6);
        match &commands[1].kind {
            CommandKind::RawCommand { path, .. } => assert_eq!(
                path,
                &["cursor".to_string(), "firstBatch".to_string()]
            ),
            _ => panic!("expected raw command"),
        }
        match &commands[5].kind {
            CommandKind::RawCommand { path, .. } => {
                assert_eq!(path, &["size".to_string()])
            }
            _ => panic!("expected raw command"),
        }

        let commands =
            parse_script("rs.hello(); rs.conf(); rs.printReplicationInfo()", "app").unwrap();
        assert_eq!(commands.len(), 3);
        assert!(commands.iter().all(|command| matches!(
            command.kind,
            CommandKind::RawCommand { admin: true, .. }
        )));
        assert!(parse_script("db.users.explain()", "app").is_err());
    }

    #[test]
    fn parses_sharding_helpers() {
        let commands = parse_script("sh.status();\nsh.getBalancerState()", "app").unwrap();
        assert_eq!(commands.len(), 2);
        assert!(matches!(commands[0].kind, CommandKind::ShStatus));
        assert!(matches!(
            commands[1].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
        let commands = parse_script("rs.status()", "app").unwrap();
        assert!(matches!(
            commands[0].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
        assert!(parse_script(
            "sh.shardCollection(\"app.users\", {\"_id\":1})",
            "app"
        )
        .is_err());
    }

    #[test]
    fn parses_status_field_paths_and_array_batches() {
        let commands = parse_script(
            "[\n  db.version(),\n  db.serverStatus().connections.current\n]",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 2);
        match &commands[1].kind {
            CommandKind::RawCommand { path, admin, .. } => {
                assert!(admin);
                assert_eq!(path, &["connections".to_string(), "current".to_string()]);
            }
            _ => panic!("expected raw command"),
        }
    }

    #[test]
    fn parses_sibling_database_scope() {
        let commands = parse_script(
            "db.getSiblingDB('admin').serverStatus();\ndb.getSiblingDB(\"app\").users.find({\"x\":1})",
            "test",
        )
        .unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].database, "admin");
        assert!(matches!(
            commands[0].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
        assert_eq!(commands[1].database, "app");
        match &commands[1].kind {
            CommandKind::Call { collection, .. } => assert_eq!(collection, "users"),
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn unwraps_mongosh_eval_invocations() {
        let commands = parse_script(
            "mongosh \"mongodb://localhost:27017\" --eval \"db.serverStatus()\"",
            "app",
        )
        .unwrap();
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            commands[0].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
        let commands = parse_script("mongo --eval='db.stats()'", "app").unwrap();
        assert!(matches!(
            commands[0].kind,
            CommandKind::RawCommand { admin: false, .. }
        ));
    }

    #[test]
    fn parses_use_function_and_get_db_names() {
        let commands =
            parse_script("use(\"analytics\");\ndb.getMongo().getDBNames()", "app").unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].database, "analytics");
        assert_eq!(commands[1].database, "analytics");
        assert!(matches!(
            commands[1].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
    }

    #[test]
    fn parses_find_chains_and_special_collection_names() {
        let commands = parse_script(
            r#"db.getCollection("a.b").find({"x":1}).sort({"x":-1}).skip(1).limit(10)"#,
            "app",
        )
        .unwrap();
        match &commands[0].kind {
            CommandKind::Call { collection, method, chain, .. } => {
                assert_eq!(collection, "a.b");
                assert_eq!(method, "find");
                assert_eq!(chain.skip, 1);
                assert_eq!(chain.limit, Some(10));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn rejects_javascript_style_arguments_and_unsupported_methods() {
        assert!(parse_script(r#"db.users.find({name:"x"})"#, "app").is_err());
        assert!(parse_script(r#"db.users.find({"name":'x'})"#, "app").is_err());
        assert!(parse_script(r#"db.users.createIndex({"a":1})"#, "app").is_err());
        assert!(parse_script(r#"db.users.drop()"#, "app").is_err());
        assert!(parse_script(
            r#"db.users.aggregate([{"$out":"archive"}])"#,
            "app"
        )
        .is_err());
    }

    #[test]
    fn detects_dangerous_empty_filters() {
        let commands = parse_script(r#"db.users.deleteMany({})"#, "app").unwrap();
        assert!(confirmation_reason(&commands[0]).is_some());
        let commands = parse_script(r#"db.users.deleteMany({"status":"x"})"#, "app").unwrap();
        assert!(confirmation_reason(&commands[0]).is_none());
    }

    #[test]
    fn use_commands_change_subsequent_database() {
        let commands = parse_script("use analytics;\ndb.users.find({})", "app").unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[1].database, "analytics");
    }

    #[tokio::test]
    #[ignore = "requires MYDB_TEST_MONGO_CONNECTION_ID pointing at a saved MongoDB connection"]
    async fn real_mongo_console_accepts_ai_style_commands() {
        let connection_id = std::env::var("MYDB_TEST_MONGO_CONNECTION_ID")
            .expect("set MYDB_TEST_MONGO_CONNECTION_ID");
        let home = std::env::var("HOME").expect("HOME");
        let path = std::path::Path::new(&home)
            .join("Library/Application Support/com.mengqi.mydb/connections.json");
        let raw = std::fs::read_to_string(&path).expect("read connections.json");
        let connections: Vec<crate::models::ConnectionConfig> =
            serde_json::from_str(&raw).expect("parse connections.json");
        let config = connections
            .into_iter()
            .find(|item| item.id == connection_id)
            .expect("saved MongoDB connection");
        assert_eq!(config.driver, "mongodb");
        let password = crate::keychain::load(&config.id, "db").expect("load password");
        let client = crate::commands::mongo::connect_with_config(
            &config,
            &config.host,
            config.port,
            password.as_deref(),
            None,
        )
        .await
        .expect("connect MongoDB");

        let script = format!(
            "printjson(db.stats());\ndb.getSiblingDB(\"{}\").getCollectionNames()",
            config.database
        );
        let commands = parse_script(&script, &config.database).unwrap();
        assert_eq!(commands.len(), 2);
        for command in &commands {
            let CommandKind::RawCommand {
                command: cmd,
                admin,
                ..
            } = &command.kind
            else {
                panic!("expected raw command");
            };
            let database = if *admin { "admin" } else { command.database.as_str() };
            let doc = client
                .database(database)
                .run_command(cmd.clone())
                .await
                .unwrap_or_else(|error| panic!("{database} {cmd:?} failed: {error}"));
            let ok = doc.get_f64("ok").unwrap_or(1.0);
            assert!(ok >= 1.0, "command did not report ok: {doc:?}");
        }

        // `mongosh --eval "db.serverStatus()"` must reach the server. Some
        // accounts lack the clusterMonitor privilege; an auth error is still
        // proof that the console no longer rejects the wrapper as a syntax
        // error.
        let wrapped = parse_script(
            "# 查看状态\nmongosh --eval \"db.serverStatus()\"",
            &config.database,
        )
        .unwrap();
        assert!(matches!(
            wrapped[0].kind,
            CommandKind::RawCommand { admin: true, .. }
        ));
        match client
            .database("admin")
            .run_command(doc! { "serverStatus": 1 })
            .await
        {
            Ok(doc) => {
                let ok = doc.get_f64("ok").unwrap_or(1.0);
                assert!(ok >= 1.0, "serverStatus did not report ok: {doc:?}");
            }
            Err(error) => {
                let message = error.to_string().to_ascii_lowercase();
                assert!(
                    message.contains("auth") || message.contains("privilege"),
                    "unexpected serverStatus error: {message}"
                );
            }
        }

        // Collection info helpers must reach the server with plain names.
        if let Ok(names) = client.database(&config.database).list_collection_names().await {
            if let Some(name) = names.into_iter().find(|name| is_collection_name(name)) {
                let stats = client
                    .database(&config.database)
                    .run_command(doc! { "collStats": &name })
                    .await
                    .expect("collStats");
                assert!(stats.contains_key("size"), "collStats missing size");
                client
                    .database(&config.database)
                    .run_command(doc! { "listIndexes": &name })
                    .await
                    .expect("listIndexes");
                let parsed =
                    parse_script(&format!("db.{name}.stats().size"), &config.database).unwrap();
                match &parsed[0].kind {
                    CommandKind::RawCommand { path, admin, .. } => {
                        assert!(!admin);
                        assert_eq!(path, &["size".to_string()]);
                    }
                    _ => panic!("expected raw command"),
                }
            }
        }

        // Sharding helpers must be accepted; on a mongod the server itself
        // rejects listShards, which is surfaced as a server error instead of
        // a console syntax error.
        let sh = parse_script("sh.status()", &config.database).unwrap();
        assert!(matches!(sh[0].kind, CommandKind::ShStatus));
        match client
            .database("admin")
            .run_command(doc! { "listShards": 1 })
            .await
        {
            Ok(document) => println!(
                "sh.status() listShards ok: {} shards",
                document
                    .get_array("shards")
                    .map(|items| items.len())
                    .unwrap_or(0)
            ),
            Err(error) => println!("sh.status() unavailable on this server: {error}"),
        }

        // The exact shape a model produced when asked for version +
        // connection counts: an array of calls with field paths.
        let batch = parse_script(
            "[\n  db.version(),\n  db.serverStatus().connections,\n  db.serverStatus().uptime\n]",
            &config.database,
        )
        .unwrap();
        assert_eq!(batch.len(), 3);
        for command in &batch {
            let CommandKind::RawCommand {
                command: cmd,
                admin,
                ..
            } = &command.kind
            else {
                panic!("expected raw command");
            };
            let database = if *admin { "admin" } else { command.database.as_str() };
            if let Err(error) = client.database(database).run_command(cmd.clone()).await {
                let message = error.to_string().to_ascii_lowercase();
                assert!(
                    message.contains("auth") || message.contains("privilege"),
                    "batch command {cmd:?} failed: {message}"
                );
            }
        }
    }
}
