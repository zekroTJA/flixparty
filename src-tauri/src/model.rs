use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectionParams {
    pub address: String,
    pub channel: String,
    pub auth: Option<ConnectionAuth>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectionAuth {
    pub username: Option<String>,
    pub password: String,
}
