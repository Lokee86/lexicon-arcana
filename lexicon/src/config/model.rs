use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::languages::{language_enabled, supported_languages};
use crate::repository::prepare_state_directory;

use super::path::config_path;

pub const CONFIG_VERSION: u64 = 1;
pub const ANALYSIS_CONFIG_ID: &str =
    "sha256:3eed35c601d346d8b9ebeef46b5e98e5e6307192a8b7676d56cf148f4034ca8a";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub version: u64,
    pub adapter_root: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_languages: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            adapter_root: String::new(),
            enabled_languages: Vec::new(),
        }
    }
}

impl Config {
    pub fn language_enabled(&self, language: &str) -> bool {
        language_enabled(language, &self.enabled_languages)
    }
}

pub fn normalize_enabled_languages(enabled_languages: &[String]) -> Result<Vec<String>, String> {
    let supported = supported_languages();
    let mut normalized = enabled_languages.to_vec();
    normalized.sort();
    normalized.dedup();
    if let Some(language) = normalized
        .iter()
        .find(|language| !supported.contains(language))
    {
        return Err(format!("unsupported Lexicon language {language:?}"));
    }
    Ok(normalized)
}

pub fn save_config(repository: &Path, adapter_root: &Path) -> Result<Config, String> {
    let enabled = load_config(repository)
        .map(|config| config.enabled_languages)
        .unwrap_or_default();
    save_value(repository, adapter_root, enabled)
}

pub fn save_config_with_languages(
    repository: &Path,
    adapter_root: &Path,
    enabled_languages: &[String],
) -> Result<Config, String> {
    save_value(
        repository,
        adapter_root,
        normalize_enabled_languages(enabled_languages)?,
    )
}

pub fn update_enabled_languages(
    repository: &Path,
    enabled_languages: &[String],
) -> Result<Config, String> {
    let mut value = load_config(repository)?;
    value.enabled_languages = normalize_enabled_languages(enabled_languages)?;
    write_config(repository, &value)?;
    Ok(value)
}

pub fn load_config(repository: &Path) -> Result<Config, String> {
    let path = config_path(repository);
    let data = fs::read(&path).map_err(|error| format!("read Lexicon configuration: {error}"))?;
    let mut value: Config = serde_json::from_slice(&data)
        .map_err(|error| format!("decode Lexicon configuration: {error}"))?;
    if value.version != CONFIG_VERSION {
        return Err(format!(
            "unsupported Lexicon configuration version {}",
            value.version
        ));
    }
    value.enabled_languages = normalize_enabled_languages(&value.enabled_languages)
        .map_err(|error| format!("validate Lexicon enabled languages: {error}"))?;
    Ok(value)
}

fn save_value(
    repository: &Path,
    adapter_root: &Path,
    enabled_languages: Vec<String>,
) -> Result<Config, String> {
    let value = Config {
        version: CONFIG_VERSION,
        adapter_root: absolute_path(adapter_root)?.to_string_lossy().into_owned(),
        enabled_languages,
    };
    write_config(repository, &value)?;
    Ok(value)
}

fn write_config(repository: &Path, value: &Config) -> Result<(), String> {
    let state = super::path::state_root(repository);
    prepare_state_directory(repository, &state).map_err(|error| error.to_string())?;
    let mut data = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("encode Lexicon configuration: {error}"))?;
    data.push(b'\n');
    fs::write(config_path(repository), data)
        .map_err(|error| format!("write Lexicon configuration: {error}"))
}

fn absolute_path(path: &Path) -> Result<std::path::PathBuf, String> {
    if path.is_absolute() {
        return Ok(super::path::clean_path(path));
    }
    std::env::current_dir()
        .map(|current| super::path::clean_path(&current.join(path)))
        .map_err(|error| format!("resolve adapter root: {error}"))
}
