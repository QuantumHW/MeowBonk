use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub listening_enabled: bool,
}

pub fn load(path: &Path) -> Result<Preferences, String> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(e) => return Err(format!("无法读取已保存的设置：{e}")),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取设置：{e}"))?;
    if bytes.len() > 4096 {
        return Err("设置文件超过大小限制，未自动启用召唤".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "设置文件格式无效，未自动启用召唤".into())
}

// Write beside the destination, flush, then replace. No partially written JSON is published.
pub fn save(path: &Path, preferences: Preferences) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = path.parent().ok_or("设置目录无效")?;
    fs::create_dir_all(directory).map_err(|e| format!("无法创建设置目录：{e}"))?;
    let temporary = directory.join(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|e| format!("无法保存设置：{e}"))?;
    let result = (|| {
        let bytes = serde_json::to_vec(&preferences).map_err(std::io::Error::other)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|e| format!("设置未保存，下次启动可能不会恢复本次选择：{e}"))
}
#[cfg(not(target_os = "windows"))]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}
#[cfg(target_os = "windows")]
fn replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub struct Store {
    pub path: PathBuf,
    pub preferences: Preferences,
}
impl Store {
    pub fn set_enabled(&mut self, enabled: bool) -> Result<(), String> {
        let next = Preferences {
            listening_enabled: enabled,
        };
        save(&self.path, next)?;
        self.preferences = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../.cache/settings-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn remembers_enabled_and_paused_across_reloads() {
        let folder = folder();
        let path = folder.join("settings.json");
        assert!(!load(&path).unwrap().listening_enabled);
        save(
            &path,
            Preferences {
                listening_enabled: true,
            },
        )
        .unwrap();
        assert!(load(&path).unwrap().listening_enabled);
        save(
            &path,
            Preferences {
                listening_enabled: false,
            },
        )
        .unwrap();
        assert!(!load(&path).unwrap().listening_enabled);
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn rejects_corrupted_and_oversized_preferences_without_replacing_them() {
        let folder = folder();
        let path = folder.join("settings.json");
        for bytes in [
            b"{bad".as_slice(),
            b"{\"listening_enabled\":\"true\"}",
            b"{\"unrecognized\":true}",
        ] {
            fs::write(&path, bytes).unwrap();
            assert!(load(&path).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        fs::write(&path, vec![b' '; 4097]).unwrap();
        assert!(load(&path).is_err());
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn failed_save_keeps_in_memory_preference_and_existing_target() {
        let folder = folder();
        let path = folder.join("settings.json");
        fs::create_dir(&path).unwrap();
        let mut store = Store {
            path: path.clone(),
            preferences: Preferences::default(),
        };
        assert!(store.set_enabled(true).is_err());
        assert!(!store.preferences.listening_enabled);
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&folder).unwrap().count(), 1);
        fs::remove_dir_all(folder).unwrap();
    }
}
