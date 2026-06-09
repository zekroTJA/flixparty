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
    // Windows: VK_MEDIA_PLAY_PAUSE (0xB3).
    #[cfg(target_os = "windows")]
    return Key::Unknown(179);

    // Linux (X11): XF86AudioPlay keycode.
    #[cfg(target_os = "linux")]
    return Key::Unknown(172);

    // macOS: the dedicated play/pause key is a system-defined (NX) event, not a
    // CGKeyCode, so rdev cannot simulate it. Space toggles play/pause in the
    // focused media player instead.
    #[cfg(target_os = "macos")]
    return Key::Space;
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
pub struct Condition {
    pub class: Option<String>,
    pub title_contains: Option<String>,
    pub block_trigger: bool,
    pub block_receive: bool,
}

impl Default for Condition {
    fn default() -> Self {
        Self {
            class: None,
            title_contains: None,
            block_trigger: true,
            block_receive: false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_loglevel")]
    pub log_level: String,
    #[serde(default)]
    pub keys: Keys,
    pub connection: Connection,
    pub condition: Condition,
}

impl Config {
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Figment::new().merge(Toml::file(path)).extract()?)
    }
}
