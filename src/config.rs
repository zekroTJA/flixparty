use anyhow::Result;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use rdev::Key;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Connection {
    pub address: String,
    pub channel: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub log_level: Option<String>,
    pub toggle_key: Option<Key>,
    pub connection: Connection,
}

impl Config {
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Figment::new().merge(Toml::file(path)).extract()?)
    }
}
