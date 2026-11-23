pub(crate) fn first_keyword(sql: &str) -> &str {
    let mut rest = sql.trim_start();

    loop {
        if rest.starts_with("--") {
            match rest.find('\n') {
                Some(position) => rest = rest[position + 1..].trim_start(),
                None => return "",
            }
            continue;
        }
        if rest.starts_with("/*") {
            match rest.find("*/") {
                Some(position) => rest = rest[position + 2..].trim_start(),
                None => return "",
            }
            continue;
        }
        break;
    }

    rest.split_whitespace().next().unwrap_or_default()
}

pub(crate) fn is_read_query(sql: &str) -> bool {
    let keyword = first_keyword(sql).to_ascii_lowercase();
    let is_read = matches!(
        keyword.as_str(),
        "select" | "with" | "pragma" | "explain" | "show" | "describe" | "desc"
    );

    is_read || sql.to_ascii_lowercase().contains(" returning ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_line_comments() {
        assert_eq!(first_keyword("-- hello\nSELECT 1"), "SELECT");
    }

    #[test]
    fn skips_block_comments() {
        assert_eq!(first_keyword("/* hello */ SELECT 1"), "SELECT");
    }

    #[test]
    fn recognizes_read_queries() {
        assert!(is_read_query("SELECT 1"));
        assert!(is_read_query(
            "-- note\nWITH x AS (SELECT 1) SELECT * FROM x"
        ));
        assert!(is_read_query("PRAGMA table_info(users)"));
        assert!(!is_read_query("INSERT INTO t VALUES (1)"));
        assert!(!is_read_query("CREATE TABLE t (id INTEGER)"));
    }
}
