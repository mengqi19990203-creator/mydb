use keyring::Entry;

const SERVICE: &str = "mydb";

fn entry(connection_id: &str, kind: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("connection:{connection_id}:{kind}"))
        .map_err(|error| format!("无法访问系统钥匙串: {error}"))
}

pub fn store(connection_id: &str, kind: &str, password: &str) -> Result<(), String> {
    entry(connection_id, kind)?
        .set_password(password)
        .map_err(|error| format!("写入系统钥匙串失败: {error}"))
}

pub fn load(connection_id: &str, kind: &str) -> Result<Option<String>, String> {
    let entry = entry(connection_id, kind)?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("读取系统钥匙串失败: {error}")),
    }
}

pub fn delete(connection_id: &str, kind: &str) {
    if let Ok(entry) = entry(connection_id, kind) {
        let _ = entry.delete_credential();
    }
}
