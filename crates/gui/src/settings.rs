//! Persists the config in the OS specific config directory, e.g.
//! `%APPDATA%\flixparty\config\config.toml` on Windows,
//! `~/.config/flixparty/config.toml` on Linux and
//! `~/Library/Application Support/flixparty/config.toml` on macOS.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use flixparty_core::Config;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

fn path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "flixparty").map(|dirs| dirs.config_dir().join("config.toml"))
}

pub fn load() -> Config {
    let Some(path) = path() else {
        warn!("could not determine config directory");
        return Config::default();
    };

    if !path.exists() {
        return Config::default();
    }

    match Config::from_file(&path) {
        Ok(cfg) => {
            info!("Loaded settings from {}", path.display());
            cfg
        }
        Err(err) => {
            warn!("failed loading settings from {}: {err}", path.display());
            Config::default()
        }
    }
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = path().context("could not determine config directory")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, toml::to_string_pretty(cfg)?)?;
    info!("Saved settings to {}", path.display());
    Ok(())
}
