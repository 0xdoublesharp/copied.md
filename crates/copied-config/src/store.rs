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

    let data = match fs::read(path) {
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
    tmp.persist(path).map_err(|error| error.error)?;
    Ok(())
}
