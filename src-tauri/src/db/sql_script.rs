#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptStatement {
    pub sql: String,
    pub line: usize,
}

pub fn split_sql_script(input: &str) -> Result<Vec<ScriptStatement>, String> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let bytes = input.as_bytes();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut statement_line = 1usize;
    let mut line = 1usize;
    let mut delimiter = ";".to_string();
    let mut paren_depth = 0i32;
    let mut trigger_depth = 0i32;
    let mut case_depth = 0i32;
    let mut seen_create = false;
    let mut trigger_statement = false;
    let mut line_has_content = false;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'\n' {
            current.push('\n');
            line += 1;
            line_has_content = false;
            index += 1;
            continue;
        }
        if byte == b'\r' {
            current.push('\r');
            index += 1;
            continue;
        }
        if byte == b' ' || byte == b'\t' {
            current.push(byte as char);
            index += 1;
            continue;
        }

        if !line_has_content {
            let line_end = input[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(input.len());
            let rest = &input[index..line_end];
            let trimmed = rest.trim_start();
            if trimmed.len() >= 9
                && trimmed[..9].eq_ignore_ascii_case("DELIMITER")
                && (trimmed.len() == 9
                    || trimmed[9..]
                        .chars()
                        .next()
                        .map(char::is_whitespace)
                        .unwrap_or(false))
            {
                let new_delimiter = trimmed[9..].trim();
                if new_delimiter.is_empty() {
                    return Err(format!("第 {line} 行 DELIMITER 缺少分隔符"));
                }
                if !current.trim().is_empty() {
                    return Err(format!("第 {line} 行 DELIMITER 必须位于语句之间并单独成行"));
                }
                delimiter = new_delimiter
                    .split_whitespace()
                    .next()
                    .unwrap_or(";")
                    .to_string();
                index = line_end;
                line_has_content = true;
                continue;
            }
            if trimmed.starts_with('\\') {
                return Err(format!("第 {line} 行包含不支持的 psql 元命令，已拒绝执行"));
            }
            line_has_content = true;
        }

        if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            let end = input[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(input.len());
            current.push_str(&input[index..end]);
            index = end;
            continue;
        }
        if byte == b'#' {
            let end = input[index..]
                .find('\n')
                .map(|offset| index + offset)
                .unwrap_or(input.len());
            current.push_str(&input[index..end]);
            index = end;
            continue;
        }
        if byte == b'/'
            && bytes.get(index + 1) == Some(&b'*')
            && !input[index..].starts_with(&delimiter)
        {
            let end = input[index + 2..]
                .find("*/")
                .map(|offset| index + 2 + offset + 2)
                .ok_or_else(|| format!("第 {line} 行块注释未闭合"))?;
            current.push_str(&input[index..end]);
            line += input[index..end].matches('\n').count();
            index = end;
            continue;
        }

        if input[index..].starts_with(&delimiter) && paren_depth == 0 && trigger_depth == 0 {
            let sql = current.trim().to_string();
            if !sql.is_empty() && started {
                statements.push(ScriptStatement {
                    sql,
                    line: statement_line,
                });
            }
            current.clear();
            started = false;
            trigger_depth = 0;
            case_depth = 0;
            seen_create = false;
            trigger_statement = false;
            paren_depth = 0;
            line += delimiter.matches('\n').count();
            line_has_content = !delimiter.ends_with('\n');
            index += delimiter.len();
            continue;
        }

        if matches!(byte, b'\'' | b'"' | b'`') {
            let end =
                skip_quoted(input, index, byte).ok_or_else(|| format!("第 {line} 行引号未闭合"))?;
            if !started {
                statement_line = line;
                started = true;
            }
            current.push_str(&input[index..end]);
            line += input[index..end].matches('\n').count();
            index = end;
            continue;
        }
        if byte == b'[' {
            let end = input[index..]
                .find(']')
                .map(|offset| index + offset + 1)
                .ok_or_else(|| format!("第 {line} 行方括号标识符未闭合"))?;
            if !started {
                statement_line = line;
                started = true;
            }
            current.push_str(&input[index..end]);
            line += input[index..end].matches('\n').count();
            index = end;
            continue;
        }
        if byte == b'$' {
            if let Some(end) = dollar_quote_end(input, index) {
                let end = end.ok_or_else(|| format!("第 {line} 行 dollar quote 未闭合"))?;
                if !started {
                    statement_line = line;
                    started = true;
                }
                current.push_str(&input[index..end]);
                line += input[index..end].matches('\n').count();
                index = end;
                continue;
            }
        }

        if is_word_start(byte) {
            let end = read_word_end(input, index);
            let word = input[index..end].to_ascii_uppercase();
            if !started {
                statement_line = line;
                started = true;
            }
            if delimiter == ";" {
                if word == "CREATE" {
                    seen_create = true;
                } else if word == "TRIGGER" && seen_create {
                    trigger_statement = true;
                } else if word == "BEGIN" && trigger_statement {
                    trigger_depth += 1;
                } else if word == "CASE" && trigger_depth > 0 {
                    case_depth += 1;
                } else if word == "END" && trigger_depth > 0 {
                    if case_depth > 0 {
                        case_depth -= 1;
                    } else {
                        trigger_depth -= 1;
                    }
                }
            }
            current.push_str(&input[index..end]);
            index = end;
            continue;
        }

        if byte == b'(' {
            paren_depth += 1;
            if !started {
                statement_line = line;
                started = true;
            }
            current.push('(');
            index += 1;
            continue;
        }
        if byte == b')' {
            paren_depth = (paren_depth - 1).max(0);
            current.push(')');
            index += 1;
            continue;
        }

        if !started {
            statement_line = line;
            started = true;
        }
        let character = input[index..].chars().next().unwrap_or_default();
        current.push(character);
        index += character.len_utf8();
    }

    if trigger_depth > 0 {
        return Err(format!("第 {statement_line} 行 SQLite 触发器缺少 END"));
    }
    let sql = current.trim().to_string();
    if !sql.is_empty() && started {
        statements.push(ScriptStatement {
            sql,
            line: statement_line,
        });
    }
    Ok(statements)
}

fn is_word_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn read_word_end(input: &str, start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut index = start;
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    index
}

fn skip_quoted(input: &str, start: usize, quote: u8) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' && quote != b'`' {
            index += 2;
            continue;
        }
        if bytes[index] == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Some(index + 1);
        }
        index += 1;
    }
    None
}

/// Returns `None` when this `$` does not start a dollar-quoted string,
/// `Some(None)` for an unterminated dollar quote, and `Some(Some(end))`
/// for the byte offset just past the closing tag.
fn dollar_quote_end(input: &str, start: usize) -> Option<Option<usize>> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'$') {
        return None;
    }
    let mut index = start + 1;
    while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'$') {
        return None;
    }
    let tag = &input[start..=index];
    let body_start = index + 1;
    input[body_start..]
        .find(tag)
        .map(|offset| Some(body_start + offset + tag.len()))
        .or(Some(None))
}

#[cfg(test)]
mod tests {
    use super::split_sql_script;

    #[test]
    fn splits_basic_statements_and_tracks_lines() {
        let statements = split_sql_script("-- setup\nSELECT 1;\n\nSELECT ';\n' AS semi;\nSELECT 3")
            .expect("split");
        assert_eq!(statements.len(), 3);
        assert_eq!(statements[0].line, 2);
        assert_eq!(statements[1].sql, "SELECT ';\n' AS semi");
        assert_eq!(statements[2].line, 6);
    }

    #[test]
    fn skips_comments_and_bracket_identifiers() {
        let statements = split_sql_script(
            "# mysql comment\nCREATE TABLE [a;b] (id INTEGER);\n/* block ; */ SELECT 1;",
        )
        .expect("split");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].sql.contains("[a;b]"));
        assert_eq!(statements[1].sql, "/* block ; */ SELECT 1");
    }

    #[test]
    fn handles_postgres_dollar_quotes() {
        let statements = split_sql_script(
            "CREATE FUNCTION f() RETURNS void AS $body$\nBEGIN\n  PERFORM 1;\nEND;\n$body$ LANGUAGE plpgsql;\nDO $$ BEGIN NULL; END $$;",
        )
        .expect("split");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].sql.contains("PERFORM 1;"));
        assert!(statements[1].sql.contains("DO $$"));
    }

    #[test]
    fn handles_mysql_delimiter_directives() {
        let statements = split_sql_script(
            "DELIMITER $$\nCREATE PROCEDURE p()\nBEGIN\n  SELECT 1;\n  SELECT 2;\nEND$$\nDELIMITER ;\nSELECT 3;",
        )
        .expect("split");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].sql.contains("SELECT 2;"));
        assert_eq!(statements[1].sql, "SELECT 3");
    }

    #[test]
    fn keeps_sqlite_trigger_bodies_together() {
        let statements = split_sql_script(
            "CREATE TRIGGER t AFTER INSERT ON x\nBEGIN\n  UPDATE x SET a = CASE WHEN b THEN 1 ELSE 2 END WHERE id = new.id;\n  INSERT INTO log VALUES (new.id);\nEND;\nSELECT 1;",
        )
        .expect("split");
        assert_eq!(statements.len(), 2);
        assert!(statements[0].sql.contains("INSERT INTO log"));
        assert_eq!(statements[1].sql, "SELECT 1");
    }

    #[test]
    fn rejects_unterminated_constructs_and_meta_commands() {
        assert!(split_sql_script("SELECT 'x").is_err());
        assert!(split_sql_script("SELECT $body$ x").is_err());
        assert!(split_sql_script("\\copy t from 'x'").is_err());
        assert!(split_sql_script("DELIMITER").is_err());
    }
}
