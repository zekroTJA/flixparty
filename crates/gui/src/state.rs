use crate::connection::TaggedEvent;
use crate::logging::LogLine;
use chrono::Local;
use flixparty_core::{Config, Event};
use freya::prelude::*;
use freya::winit::window::WindowId;
use futures_channel::mpsc::UnboundedReceiver;
use futures_lite::StreamExt;
use rdev::Key;
use std::collections::VecDeque;

const MAX_LOG_LINES: usize = 2000;
const MAX_ACTIVITY: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Settings,
    Session,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum KeyTarget {
    Toggle,
    Playback,
}

/// The values of the settings form.
#[derive(Clone, Copy)]
pub struct Form {
    pub toggle_key: State<Key>,
    pub playback_key: State<Key>,
    /// Which key is currently being recorded.
    pub recording: State<Option<KeyTarget>>,
    /// Whether to mark required fields that are empty, set after a connect
    /// attempt with missing values.
    pub show_missing: State<bool>,
    pub address: State<String>,
    pub tls: State<bool>,
    pub username: State<String>,
    pub password: State<String>,
    pub channel: State<String>,
    pub name: State<String>,
    pub class: State<String>,
    pub title_contains: State<String>,
}

impl Form {
    fn new(cfg: &Config) -> Self {
        let c = &cfg.connection;
        Self {
            toggle_key: State::create_global(cfg.keys.toggle),
            playback_key: State::create_global(cfg.keys.playback),
            recording: State::create_global(None),
            show_missing: State::create_global(false),
            address: State::create_global(c.address.clone()),
            tls: State::create_global(c.tls),
            username: State::create_global(c.username.clone().unwrap_or_default()),
            password: State::create_global(c.password.clone().unwrap_or_default()),
            channel: State::create_global(c.channel.clone()),
            name: State::create_global(c.name.clone().unwrap_or_default()),
            class: State::create_global(cfg.condition.class.clone().unwrap_or_default()),
            title_contains: State::create_global(
                cfg.condition.title_contains.clone().unwrap_or_default(),
            ),
        }
    }

    /// Applies the form values on top of `base`, which holds the settings that
    /// are not part of the form.
    pub fn to_config(self, base: &Config) -> Config {
        let mut cfg = base.clone();
        cfg.keys.toggle = *self.toggle_key.read();
        cfg.keys.playback = *self.playback_key.read();

        let c = &mut cfg.connection;
        c.address = self.address.read().trim().to_string();
        c.tls = *self.tls.read();
        c.username = non_empty(&self.username.read());
        c.password = non_empty(&self.password.read());
        c.channel = non_empty(&self.channel.read()).unwrap_or_else(|| "flixparty".into());
        c.name = non_empty(&self.name.read());

        cfg.condition.class = non_empty(&self.class.read());
        cfg.condition.title_contains = non_empty(&self.title_contains.read());
        cfg
    }
}

fn non_empty(v: &str) -> Option<String> {
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_string())
}

#[derive(Clone, PartialEq)]
pub enum Status {
    Connecting,
    Connected,
    Reconnecting { error: String, remaining: usize },
    Disconnected,
}

#[derive(Clone, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub is_self: bool,
    /// Time of the last toggle triggered by this member.
    pub last_toggle: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ActivityKind {
    Toggle,
    Info,
    Warning,
}

#[derive(Clone, PartialEq)]
pub struct Activity {
    pub time: String,
    pub text: String,
    pub kind: ActivityKind,
}

#[derive(Clone)]
pub struct SessionInfo {
    pub generation: u64,
    pub status: Status,
    pub channel: String,
    /// The member list. The own client always comes first.
    pub members: Vec<Member>,
    /// ID of the member who triggered the last toggle.
    pub last_toggle_by: Option<String>,
    /// Newest first.
    pub activity: VecDeque<Activity>,
}

impl SessionInfo {
    pub fn new(generation: u64, channel: String, client_id: String, name: String) -> Self {
        Self {
            generation,
            status: Status::Connecting,
            channel,
            members: vec![Member {
                id: client_id,
                name,
                is_self: true,
                last_toggle: None,
            }],
            last_toggle_by: None,
            activity: VecDeque::new(),
        }
    }

    fn push_activity(&mut self, kind: ActivityKind, text: String) {
        self.activity.push_front(Activity {
            time: now(),
            text,
            kind,
        });
        self.activity.truncate(MAX_ACTIVITY);
    }

    fn member_name(&self, id: &str) -> Option<String> {
        self.members
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.name.clone())
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Connecting => {
                if !matches!(self.status, Status::Reconnecting { .. }) {
                    self.status = Status::Connecting;
                }
            }
            Event::Connected => {
                self.status = Status::Connected;
                self.push_activity(
                    ActivityKind::Info,
                    format!("Joined channel {}", self.channel),
                );
            }
            Event::ConnectionLost {
                error,
                remaining_retries,
            } => {
                self.push_activity(ActivityKind::Warning, format!("Connection lost: {error}"));
                self.status = Status::Reconnecting {
                    error,
                    remaining: remaining_retries,
                };
            }
            Event::MemberJoined { id, name } => {
                if let Some(member) = self.members.iter_mut().find(|m| m.id == id) {
                    member.name = name;
                } else {
                    self.push_activity(ActivityKind::Info, format!("{name} joined"));
                    self.members.push(Member {
                        id,
                        name,
                        is_self: false,
                        last_toggle: None,
                    });
                }
            }
            Event::MemberLeft { id } => {
                if let Some(pos) = self.members.iter().position(|m| m.id == id && !m.is_self) {
                    let member = self.members.remove(pos);
                    self.push_activity(ActivityKind::Info, format!("{} left", member.name));
                }
            }
            Event::Toggle {
                sender,
                name,
                is_self,
                applied,
                round_trip,
            } => {
                let name = if is_self {
                    "You".to_string()
                } else {
                    name.or_else(|| self.member_name(&sender))
                        .unwrap_or_else(|| sender.clone())
                };

                let mut text = format!("{name} toggled playback");
                if let Some(rtt) = round_trip {
                    text.push_str(&format!(" ({} ms round trip)", rtt.as_millis()));
                }
                if !applied {
                    text.push_str(" (ignored: no matching window in focus)");
                }
                self.push_activity(ActivityKind::Toggle, text);

                let time = now();
                if let Some(member) = self.members.iter_mut().find(|m| m.id == sender) {
                    member.last_toggle = Some(time);
                }
                self.last_toggle_by = Some(sender);
            }
            Event::TriggerBlocked => self.push_activity(
                ActivityKind::Warning,
                "Toggle key ignored: no matching window in focus".into(),
            ),
            Event::Stopped { .. } => self.status = Status::Disconnected,
        }
    }
}

fn now() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

#[derive(Clone, Copy)]
pub struct Globals {
    /// The last loaded or saved config.
    pub config: State<Config>,
    pub form: Form,
    pub view: State<View>,
    /// Shown in the settings view, e.g. when connecting failed.
    pub error: State<Option<String>>,
    pub session: State<Option<SessionInfo>>,
    pub logs: State<VecDeque<LogLine>>,
    pub log_window: State<Option<WindowId>>,
}

impl Globals {
    pub fn new(cfg: Config) -> Self {
        Self {
            form: Form::new(&cfg),
            config: State::create_global(cfg),
            view: State::create_global(View::Settings),
            error: State::create_global(None),
            session: State::create_global(None),
            logs: State::create_global(VecDeque::new()),
            log_window: State::create_global(None),
        }
    }
}

pub async fn pump_logs(mut rx: UnboundedReceiver<LogLine>, mut logs: State<VecDeque<LogLine>>) {
    while let Some(line) = rx.next().await {
        let mut logs = logs.write();
        logs.push_back(line);
        if logs.len() > MAX_LOG_LINES {
            logs.pop_front();
        }
    }
}

pub async fn pump_events(mut rx: UnboundedReceiver<TaggedEvent>, mut g: Globals) {
    while let Some((generation, event)) = rx.next().await {
        let mut session = g.session.write();
        let Some(info) = session.as_mut().filter(|s| s.generation == generation) else {
            continue;
        };

        if let Event::Stopped { error: Some(err) } = &event {
            g.error.set(Some(err.clone()));
            g.view.set(View::Settings);
        }

        info.apply(event);
    }
}

/// Applies key presses to the form while a key is being recorded.
pub async fn pump_keys(mut rx: UnboundedReceiver<Key>, mut form: Form) {
    while let Some(key) = rx.next().await {
        let Some(target) = *form.recording.peek() else {
            continue;
        };
        match target {
            KeyTarget::Toggle => form.toggle_key.set(key),
            KeyTarget::Playback => form.playback_key.set(key),
        }
        form.recording.set(None);
    }
}
