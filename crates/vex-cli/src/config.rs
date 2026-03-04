use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CliConfig {
    pub api_key: String,
    pub server_url: String,
}

fn config_path() -> Result<PathBuf> {
    let home = dirs_path()?;
    Ok(home.join("config.json"))
}

fn dirs_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let dir = PathBuf::from(home).join(".vex");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn load() -> Result<CliConfig> {
    let path = config_path()?;
    let content =
        std::fs::read_to_string(&path).context("not authenticated, run `vex login` first")?;
    serde_json::from_str(&content).context("invalid config file")
}

pub fn save(config: &CliConfig) -> Result<()> {
    let path = config_path()?;
    let content = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, content)?;
    Ok(())
}
