use anyhow::Result;
use figment::providers::{Format, Toml};
use figment::Figment;
use rdev::Key;
use serde::Deserialize;
use std::path::Path;

fn default_toggle_key() -> Key {
    Key::KeyP
}

fn default_playback_key() -> Key {
    Key::Unknown(179)
}

fn default_channel() -> String {
    "flixparty".to_string()
}

fn default_loglevel() -> String {
    "info".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Connection {
    pub address: String,
    #[serde(default = "default_channel")]
    pub channel: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Keys {
    #[serde(default = "default_toggle_key")]
    pub toggle: Key,
    #[serde(default = "default_playback_key")]
    pub playback: Key,
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            toggle: default_toggle_key(),
            playback: default_playback_key(),
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Class(String),
    TitleContains(String),
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_loglevel")]
    pub log_level: String,
    #[serde(default)]
    pub keys: Keys,
    pub connection: Connection,

    #[cfg(windows)]
    pub condition: Option<Condition>,
}

impl Config {
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Figment::new().merge(Toml::file(path)).extract()?)
    }
}
