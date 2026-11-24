use keyring::Entry;

const SERVICE: &str = "mydb";

fn entry(connection_id: &str, kind: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("connection:{connection_id}:{kind}"))
        .map_err(|error| format!("无法访问系统钥匙串: {error}"))
}

#[cfg_attr(all(target_os = "macos", debug_assertions), allow(dead_code))]
fn keyring_store(connection_id: &str, kind: &str, password: &str) -> Result<(), String> {
    entry(connection_id, kind)?
        .set_password(password)
        .map_err(|error| format!("写入系统钥匙串失败: {error}"))
}

#[cfg_attr(all(target_os = "macos", debug_assertions), allow(dead_code))]
fn keyring_load(connection_id: &str, kind: &str) -> Result<Option<String>, String> {
    let entry = entry(connection_id, kind)?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("读取系统钥匙串失败: {error}")),
    }
}

#[cfg_attr(all(target_os = "macos", debug_assertions), allow(dead_code))]
fn keyring_delete(connection_id: &str, kind: &str) {
    if let Ok(entry) = entry(connection_id, kind) {
        let _ = entry.delete_credential();
    }
}

/// Development-only credential fallback for macOS.
///
/// Each `tauri dev` rebuild produces a new ad-hoc-signed binary, which makes
/// the System Keychain treat it as a different application and prompt for the
/// login keychain password even though the database password is already saved.
/// Debug builds therefore use a user-only (0600) JSON file under the app
/// config directory. Release builds continue to use the Keychain.
#[cfg(all(target_os = "macos", debug_assertions))]
mod dev_store {
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    fn credentials_path() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME").ok_or_else(|| "无法定位用户目录".to_string())?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.mengqi.mydb")
            .join("dev-credentials.json"))
    }

    fn key(connection_id: &str, kind: &str) -> String {
        format!("connection:{connection_id}:{kind}")
    }

    fn load_map() -> Result<BTreeMap<String, String>, String> {
        load_map_at(&credentials_path()?)
    }

    fn load_map_at(path: &Path) -> Result<BTreeMap<String, String>, String> {
        if !path.exists() {
            return Ok(BTreeMap::new());
        }
        let raw =
            fs::read_to_string(path).map_err(|error| format!("读取开发版凭据失败: {error}"))?;
        if raw.trim().is_empty() {
            return Ok(BTreeMap::new());
        }
        serde_json::from_str(&raw).map_err(|error| format!("解析开发版凭据失败: {error}"))
    }

    fn save_map(map: &BTreeMap<String, String>) -> Result<(), String> {
        save_map_at(&credentials_path()?, map)
    }

    fn save_map_at(path: &Path, map: &BTreeMap<String, String>) -> Result<(), String> {
        let dir = path
            .parent()
            .ok_or_else(|| "开发版凭据路径无效".to_string())?;
        fs::create_dir_all(dir).map_err(|error| format!("创建凭据目录失败: {error}"))?;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));

        let temp = dir.join(format!(
            ".dev-credentials.tmp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|error| format!("创建开发版凭据失败: {error}"))?;
        serde_json::to_writer_pretty(&mut file, map)
            .map_err(|error| format!("写入开发版凭据失败: {error}"))?;
        file.flush()
            .map_err(|error| format!("写入开发版凭据失败: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("写入开发版凭据失败: {error}"))?;
        let _ = fs::set_permissions(&temp, fs::Permissions::from_mode(0o600));
        fs::rename(&temp, &path).map_err(|error| {
            let _ = fs::remove_file(&temp);
            format!("写入开发版凭据失败: {error}")
        })?;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        Ok(())
    }

    pub fn store(connection_id: &str, kind: &str, password: &str) -> Result<(), String> {
        let _guard = LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut map = load_map()?;
        map.insert(key(connection_id, kind), password.to_string());
        save_map(&map)
    }

    pub fn load(connection_id: &str, kind: &str) -> Result<Option<String>, String> {
        let _guard = LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        Ok(load_map()?.get(&key(connection_id, kind)).cloned())
    }

    pub fn delete(connection_id: &str, kind: &str) {
        let _guard = LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let Ok(mut map) = load_map() else {
            return;
        };
        if map.remove(&key(connection_id, kind)).is_some() {
            let _ = save_map(&map);
        }
    }

    /// Copy a credential that is still only in the Keychain into the dev file.
    ///
    /// `/usr/bin/security` is a stable Apple-signed tool and is already in the
    /// items' partition list, so this migration does not trigger the "new
    /// binary" Keychain prompt. Failures are ignored so the app can continue
    /// and ask the user to re-enter the password once in the dialog.
    pub fn migrate_from_keyring(connection_id: &str, kind: &str) -> Option<String> {
        let account = format!("connection:{connection_id}:{kind}");
        let output = Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                super::SERVICE,
                "-a",
                &account,
                "-w",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let password = String::from_utf8(output.stdout).ok()?;
        let password = password.trim_end_matches(['\r', '\n']).to_string();
        if password.is_empty() {
            return None;
        }
        if store(connection_id, kind, &password).is_ok() {
            Some(password)
        } else {
            None
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn writes_credentials_with_user_only_permissions() {
            let dir = tempfile::tempdir().expect("temp dir");
            let path = dir.path().join("dev-credentials.json");
            let mut map = BTreeMap::new();
            map.insert("connection:a:db".to_string(), "secret".to_string());
            save_map_at(&path, &map).expect("save");
            assert_eq!(
                load_map_at(&path).expect("load").get("connection:a:db"),
                Some(&"secret".to_string())
            );
            let mode = fs::metadata(&path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}

pub fn store(connection_id: &str, kind: &str, password: &str) -> Result<(), String> {
    #[cfg(all(target_os = "macos", debug_assertions))]
    {
        return dev_store::store(connection_id, kind, password);
    }
    #[cfg(not(all(target_os = "macos", debug_assertions)))]
    {
        keyring_store(connection_id, kind, password)
    }
}

pub fn load(connection_id: &str, kind: &str) -> Result<Option<String>, String> {
    #[cfg(all(target_os = "macos", debug_assertions))]
    {
        if let Some(password) = dev_store::load(connection_id, kind)? {
            return Ok(Some(password));
        }
        // One-time migration from the Keychain used by earlier builds and by
        // release builds. A denied prompt simply leaves the file empty.
        return Ok(dev_store::migrate_from_keyring(connection_id, kind));
    }
    #[cfg(not(all(target_os = "macos", debug_assertions)))]
    {
        keyring_load(connection_id, kind)
    }
}

pub fn delete(connection_id: &str, kind: &str) {
    #[cfg(all(target_os = "macos", debug_assertions))]
    {
        dev_store::delete(connection_id, kind);
    }
    #[cfg(not(all(target_os = "macos", debug_assertions)))]
    {
        keyring_delete(connection_id, kind);
    }
}