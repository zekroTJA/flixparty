use serde::{Deserialize, Serialize};

/// Operations sent over the main channel.
///
/// Older clients fail on unknown variants, so don't add new ones here. Use the
/// presence channel ([`PresenceMessage`]) for new kinds of messages instead.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    TogglePlay,
    Introduce,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    pub sender: String,
    pub op: Op,
    /// Display name of the sender. Not sent by older clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn from_json(msg: &str) -> Result<Self, serde_json::Error> {
        serde_json::de::from_str(msg)
    }

    pub fn to_json(&self) -> String {
        serde_json::ser::to_string(self).expect("encoding message to JSON")
    }
}

/// Operations sent over the presence channel (`<channel>:presence`).
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceOp {
    /// Sent on connect. Every other client answers with [`PresenceOp::Here`].
    Hello,
    /// Answer to [`PresenceOp::Hello`]; also sent periodically as heartbeat.
    Here,
    /// Sent on disconnect.
    Bye,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PresenceMessage {
    pub sender: String,
    pub name: String,
    pub op: PresenceOp,
}

impl PresenceMessage {
    pub fn from_json(msg: &str) -> Result<Self, serde_json::Error> {
        serde_json::de::from_str(msg)
    }

    pub fn to_json(&self) -> String {
        serde_json::ser::to_string(self).expect("encoding message to JSON")
    }
}

pub fn presence_channel(channel: &str) -> String {
    format!("{channel}:presence")
}
