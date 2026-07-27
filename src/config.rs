use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub socks: Option<String>,
    pub circuits: Option<usize>,
    pub retries: Option<u32>,
    pub timeout: Option<u64>,
    pub chunk_size_mb: Option<u64>,
    pub user_agent: Option<String>,
}

impl ConfigFile {
    pub fn load(explicit_path: &Option<String>) -> Result<ConfigFile> {
        if let Some(path) = explicit_path {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("could not read config file {path:?}"))?;
            return toml::from_str(&text)
                .with_context(|| format!("could not parse config file {path:?}"));
        }

        if let Some(default_path) = Self::default_path() {
            if default_path.exists() {
                let text = std::fs::read_to_string(&default_path)
                    .with_context(|| format!("could not read config file {default_path:?}"))?;
                return toml::from_str(&text)
                    .with_context(|| format!("could not parse config file {default_path:?}"));
            }
        }

        Ok(ConfigFile::default())
    }

    fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join(".onionrush.toml"))
    }
}