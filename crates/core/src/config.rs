use anyhow::{Context, Result};
use figment::Figment;
use figment::providers::{Format, Toml};
use rdev::Key;
use redis::{ConnectionAddr, ConnectionInfo, IntoConnectionInfo};
use serde::{Deserialize, Serialize};
use std::path::Path;

const DEFAULT_REDIS_PORT: u16 = 6379;

pub fn default_toggle_key() -> Key {
    Key::KeyP
}

pub fn default_playback_key() -> Key {
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Either `host:port` or a full `redis://` / `rediss://` URL.
    pub address: String,
    /// Use TLS when `address` is given as `host:port`.
    #[serde(default)]
    pub tls: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default = "default_channel")]
    pub channel: String,
    /// The name shown to other clients. Defaults to the OS user name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            address: String::new(),
            tls: false,
            username: None,
            password: None,
            channel: default_channel(),
            name: None,
        }
    }
}

impl Connection {
    pub fn display_name(&self) -> String {
        non_empty(&self.name)
            .map(str::to_string)
            .or_else(|| std::env::var("USERNAME").ok())
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_else(|| "anonymous".into())
    }

    pub fn connection_info(&self) -> Result<ConnectionInfo> {
        let address = self.address.trim();
        if address.is_empty() {
            anyhow::bail!("no Redis address specified");
        }

        let info = if address.contains("://") {
            let info = address.into_connection_info()?;
            match info.addr().clone() {
                ConnectionAddr::Tcp(host, port) if self.tls => info.set_addr(tls_addr(host, port)),
                _ => info,
            }
        } else {
            let (host, port) = split_host_port(address)?;
            let addr =
                if self.tls { tls_addr(host, port) } else { ConnectionAddr::Tcp(host, port) };
            addr.into_connection_info()?
        };

        let mut settings = info.redis_settings().clone();
        if let Some(username) = non_empty(&self.username) {
            settings = settings.set_username(username);
        }
        if let Some(password) = non_empty(&self.password) {
            settings = settings.set_password(password);
        }

        Ok(info.set_redis_settings(settings))
    }
}

fn tls_addr(host: String, port: u16) -> ConnectionAddr {
    ConnectionAddr::TcpTls {
        host,
        port,
        insecure: false,
        tls_params: None,
    }
}

fn split_host_port(address: &str) -> Result<(String, u16)> {
    // IPv6 in brackets, e.g. "[::1]:6379".
    if let Some(rest) = address.strip_prefix('[') {
        let (host, rest) = rest.split_once(']').context("invalid IPv6 address")?;
        let port = match rest.strip_prefix(':') {
            Some(port) => port.parse().context("invalid port")?,
            None => DEFAULT_REDIS_PORT,
        };
        return Ok((host.to_string(), port));
    }

    match address.rsplit_once(':') {
        Some((host, port)) => Ok((host.to_string(), port.parse().context("invalid port")?)),
        None => Ok((address.to_string(), DEFAULT_REDIS_PORT)),
    }
}

fn non_empty(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Condition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    #[serde(default = "default_true")]
    pub block_trigger: bool,
    #[serde(default)]
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

impl Condition {
    pub fn matches(&self) -> bool {
        crate::condition::is_browser_in_focus(
            non_empty(&self.class),
            non_empty(&self.title_contains),
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_loglevel")]
    pub log_level: String,
    #[serde(default)]
    pub keys: Keys,
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub condition: Condition,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_level: default_loglevel(),
            keys: Keys::default(),
            connection: Connection::default(),
            condition: Condition::default(),
        }
    }
}

impl Config {
    pub fn from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        Ok(Figment::new().merge(Toml::file(path)).extract()?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.keys.playback == self.keys.toggle {
            anyhow::bail!("playback and toggle key must not be the same key")
        }
        self.connection.connection_info()?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn host_port() {
        assert_eq!(
            split_host_port("example.com:1234").unwrap(),
            ("example.com".into(), 1234)
        );
        assert_eq!(
            split_host_port("example.com").unwrap(),
            ("example.com".into(), DEFAULT_REDIS_PORT)
        );
        assert_eq!(split_host_port("[::1]:1234").unwrap(), ("::1".into(), 1234));
        assert!(split_host_port("example.com:abc").is_err());
    }

    #[test]
    fn connection_info_credentials() {
        let conn = Connection {
            address: "127.0.0.1:16379".into(),
            username: Some("user".into()),
            password: Some("p@ss:word/".into()),
            tls: true,
            ..Default::default()
        };
        let info = conn.connection_info().unwrap();
        assert!(matches!(
            info.addr(),
            ConnectionAddr::TcpTls { port: 16379, .. }
        ));
        assert_eq!(info.redis_settings().username(), Some("user"));
        assert_eq!(info.redis_settings().password(), Some("p@ss:word/"));
    }

    #[test]
    fn connection_info_legacy_url() {
        let conn = Connection {
            address: "redis://:foobar@127.0.0.1:16379/0".into(),
            ..Default::default()
        };
        let info = conn.connection_info().unwrap();
        assert!(matches!(info.addr(), ConnectionAddr::Tcp(_, 16379)));
        assert_eq!(info.redis_settings().password(), Some("foobar"));
    }
}

#[cfg(test)]
mod serde_test {
    use super::*;

    #[test]
    fn toml_roundtrip() {
        let mut cfg = Config::default();
        cfg.keys.playback = Key::Unknown(179);
        cfg.connection.address = "example.com:6379".into();
        cfg.connection.password = Some("secret".into());
        cfg.condition.title_contains = Some("Netflix".into());

        let saved = toml::to_string_pretty(&cfg).unwrap();
        let loaded: Config = toml::from_str(&saved).unwrap();

        assert_eq!(loaded.keys.playback, Key::Unknown(179));
        assert_eq!(loaded.keys.toggle, Key::KeyP);
        assert_eq!(loaded.connection.password.as_deref(), Some("secret"));
        assert_eq!(loaded.condition.title_contains.as_deref(), Some("Netflix"));
        assert!(loaded.condition.block_trigger);
    }
}
