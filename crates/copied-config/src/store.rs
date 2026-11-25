use crate::schema::check_schema_version;
use crate::{ConfigError, ConfigV2};
use directories::ProjectDirs;
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Returns the desktop config path for the current user.
pub fn default_config_path() -> Option<PathBuf> {
    ProjectDirs::from("md", "copied", "copied.md").map(|dirs| dirs.config_dir().join("config.json"))
}

/// Loads and normalizes a v2 config, returning defaults if the file is missing.
pub fn load_config(path: &Path) -> Result<ConfigV2, ConfigError> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SchemaHeader {
        schema_version: u8,
    }

    let data = match read_file(path) {
        Ok(data) => data,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ConfigV2::default()),
        Err(error) => return Err(error.into()),
    };
    // Check the version before interpreting fields that may have changed.
    let header: SchemaHeader = serde_json::from_slice(&data)?;
    check_schema_version(header.schema_version)?;
    let mut config: ConfigV2 = serde_json::from_slice(&data)?;
    config.normalize();
    Ok(config)
}

/// Normalizes and atomically replaces the config with a temporary file in the same directory.
pub fn save_config(path: &Path, config: &ConfigV2) -> Result<(), ConfigError> {
    check_schema_version(config.schema_version)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut config = config.clone();
    config.normalize();
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(tmp.as_file_mut(), &config)?;
    tmp.as_file().sync_all()?;
    persist_file(tmp, path)?;
    Ok(())
}

#[cfg(not(windows))]
fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    fs::read(path)
}

#[cfg(not(windows))]
fn persist_file(tmp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    tmp.persist(path).map(|_| ()).map_err(|error| error.error)
}

#[cfg(windows)]
fn is_windows_retryable(error: &std::io::Error) -> bool {
    // Windows rejects concurrent ReplaceFile/read handles with ACCESS_DENIED (5),
    // SHARING_VIOLATION (32), or LOCK_VIOLATION (33) until the peer handle closes.
    error.kind() == ErrorKind::PermissionDenied || matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(windows)]
fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut attempts = 0_u32;
    loop {
        match fs::read(path) {
            Ok(data) => return Ok(data),
            Err(error) if is_windows_retryable(&error) && attempts < 15 => {
                std::thread::sleep(std::time::Duration::from_millis(1 << attempts.min(4)));
                attempts += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(windows)]
fn persist_file(mut tmp: tempfile::NamedTempFile, path: &Path) -> std::io::Result<()> {
    let mut attempts = 0_u32;
    loop {
        match tmp.persist(path) {
            Ok(_) => return Ok(()),
            Err(error) if is_windows_retryable(&error.error) && attempts < 15 => {
                tmp = error.file;
                std::thread::sleep(std::time::Duration::from_millis(1 << attempts.min(4)));
                attempts += 1;
            }
            Err(error) => return Err(error.error),
        }
    }
}
