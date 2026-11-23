/// Conservative read-only SQL classifier.
///
/// `is_read_query` already returns true for `DELETE ... RETURNING` because of
/// its `contains(" returning ")` heuristic, so it must not be reused here.
/// This classifier strips comments/strings, rejects multiple statements, and
/// only allows SELECT / SHOW / DESCRIBE / EXPLAIN / PRAGMA forms.
pub fn ensure_read_only_sql(sql: &str, _dialect: &str) -> Result<(), String> {
    let stripped = strip_sql(sql)?;
    let statements: Vec<&str> = stripped
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if statements.is_empty() {
        return Err("SQL 不能为空".to_string());
    }
    if statements.len() > 1 {
        return Err("只读模式不允许执行多条语句".to_string());
    }

    let text = statements[0];
    let tokens = tokens(text);
    let Some(first) = tokens.first().map(String::as_str) else {
        return Err("无法识别的 SQL".to_string());
    };

    if matches!(first, "show" | "describe" | "desc") {
        return Ok(());
    }
    if first == "pragma" {
        if text.contains('=') {
            return Err("只读模式不允许修改 PRAGMA".to_string());
        }
        return Ok(());
    }
    if first == "explain" {
        if tokens.iter().any(|token| token == "analyze") {
            return Err("只读模式不允许 EXPLAIN ANALYZE".to_string());
        }
    } else if first == "with" {
        if !tokens.iter().any(|token| token == "select") {
            return Err(read_only_message());
        }
    } else if first != "select" {
        return Err(read_only_message());
    }

    const FORBIDDEN: &[&str] = &[
        "insert",
        "update",
        "delete",
        "merge",
        "create",
        "alter",
        "drop",
        "truncate",
        "grant",
        "revoke",
        "call",
        "do",
        "exec",
        "execute",
        "copy",
        "vacuum",
        "reindex",
        "attach",
        "detach",
        "load",
        "outfile",
        "dumpfile",
        "nextval",
        "setval",
        "share",
        "pg_terminate_backend",
        "pg_cancel_backend",
    ];
    if let Some(word) = tokens
        .iter()
        .find(|token| FORBIDDEN.contains(&token.as_str()))
    {
        return Err(format!("只读模式不允许该操作：{word}"));
    }

    Ok(())
}

fn read_only_message() -> String {
    "只读模式仅允许 SELECT / SHOW / DESCRIBE / EXPLAIN / PRAGMA 只读语句".to_string()
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|character: char| {
        !(character.is_ascii_alphanumeric() || character == '_')
    })
    .filter(|token| !token.is_empty())
    .map(|token| token.to_ascii_lowercase())
    .collect()
}

/// Removes comments and the contents of string/identifier quotes so keyword
/// scanning cannot be fooled by `'delete'` or `"update"`. Returns an error for
/// dollar-quoted strings, which are hard to classify safely.
fn strip_sql(sql: &str) -> Result<String, String> {
    let bytes = sql.as_bytes();
    let mut output = String::with_capacity(sql.len());
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'-' if index + 1 < bytes.len() && bytes[index + 1] == b'-' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                output.push(' ');
            }
            b'#' => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                output.push(' ');
            }
            b'/' if index + 1 < bytes.len() && bytes[index + 1] == b'*' => {
                index += 2;
                while index + 1 < bytes.len()
                    && !(bytes[index] == b'*' && bytes[index + 1] == b'/')
                {
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
                output.push(' ');
            }
            quote @ (b'\'' | b'"' | b'`') => {
                index += 1;
                while index < bytes.len() {
                    if quote == b'\'' && bytes[index] == b'\\' {
                        index += 2;
                        continue;
                    }
                    if bytes[index] == quote {
                        if index + 1 < bytes.len() && bytes[index + 1] == quote {
                            index += 2;
                            continue;
                        }
                        index += 1;
                        break;
                    }
                    index += 1;
                }
                output.push(' ');
            }
            b'[' => {
                index += 1;
                while index < bytes.len() && bytes[index] != b']' {
                    index += 1;
                }
                index = (index + 1).min(bytes.len());
                output.push(' ');
            }
            b'$' => {
                // Reject $$...$$ and $tag$...$tag$ rather than guess.
                let mut cursor = index + 1;
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                if cursor < bytes.len() && bytes[cursor] == b'$' {
                    return Err("只读模式不支持 dollar-quoted 语句".to_string());
                }
                output.push('$');
                index += 1;
            }
            byte => {
                if byte.is_ascii() {
                    output.push(byte as char);
                } else {
                    output.push(' ');
                }
                index += 1;
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_read_statements() {
        assert!(ensure_read_only_sql("SELECT 1", "sqlite").is_ok());
        assert!(ensure_read_only_sql(
            "-- note\nWITH x AS (SELECT 1) SELECT * FROM x",
            "postgres"
        )
        .is_ok());
        assert!(ensure_read_only_sql("PRAGMA table_info(t)", "sqlite").is_ok());
        assert!(ensure_read_only_sql("SHOW CREATE TABLE t", "mysql").is_ok());
        assert!(ensure_read_only_sql(
            "SELECT 'delete' AS note FROM t",
            "postgres"
        )
        .is_ok());
    }

    #[test]
    fn rejects_write_statements() {
        for sql in [
            "DELETE FROM t RETURNING *",
            "WITH x AS (DELETE FROM t RETURNING *) SELECT 1",
            "UPDATE t SET a = 1",
            "INSERT INTO t VALUES (1)",
            "CREATE TABLE x (id int)",
            "SELECT * FROM t FOR UPDATE",
            "SELECT * INTO OUTFILE '/tmp/x' FROM t",
            "SELECT nextval('s')",
            "EXPLAIN ANALYZE DELETE FROM t",
            "PRAGMA journal_mode=WAL",
            "SELECT 1; DROP TABLE t",
            "SELECT $$ delete $$",
        ] {
            assert!(
                ensure_read_only_sql(sql, "postgres").is_err(),
                "should reject: {sql}"
            );
        }
    }
}