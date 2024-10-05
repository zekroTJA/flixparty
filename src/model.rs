use serde::{Deserialize, Serialize};

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
}

impl Message {
    pub fn from_json(msg: &str) -> Result<Self, serde_json::Error> {
        serde_json::de::from_str(msg)
    }

    pub fn to_json(&self) -> String {
        serde_json::ser::to_string(self).unwrap()
    }
}
