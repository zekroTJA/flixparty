use crate::client::model::{ConnectionParams, Message, Op};
use anyhow::Result;
use redis::{Commands as _, IntoConnectionInfo};
use std::thread;

mod model;

#[derive(Default)]
pub struct Client {
    id: xid::Id,
    conn: Connection,
}

impl Client {
    pub fn connect(&mut self, params: ConnectionParams) -> Result<()> {
        todo!()
    }

    fn connect_internal(&mut self, dsn: impl IntoConnectionInfo, channel: &str) -> Result<()> {
        let client = redis::Client::open(dsn)?;

        let mut pub_con = client.get_connection()?;

        let msg = Message {
            sender: self.id.to_string(),
            op: Op::Introduce,
        };
        let _: () = pub_con.publish(channel, msg.to_json())?;

        {
            let client_id = self.id.to_string();
            let channel = channel.to_string();
            let mut sub_con = client.get_connection()?;

            thread::spawn(move || {
                let mut pubsub = sub_con.as_pubsub();
                pubsub.subscribe(channel).unwrap();

                loop {
                    let msg = match pubsub.get_message() {
                        Ok(v) => v,
                        Err(err) => {
                            tracing::error!("failed metting message: {err}");
                            // TODO: error event
                            break;
                        }
                    };
                    let payload: String = match msg.get_payload() {
                        Ok(v) => v,
                        Err(err) => {
                            tracing::error!("failed getting message playload: {err}");
                            // TODO: error event
                            break;
                        }
                    };

                    let msg = match Message::from_json(&payload) {
                        Ok(v) => v,
                        Err(err) => {
                            tracing::error!("failed getting message playload: {err}");
                            // TODO: error event
                            break;
                        }
                    };

                    match msg.op {
                        Op::TogglePlay => {
                            if msg.sender == client_id {}
                            // TODO: simulate keypress
                        }
                        Op::Introduce => {
                            // TODO: idk write to some log or whatever
                        }
                    }
                }
            });
        };

        todo!()
    }
}

#[derive(Default)]
pub enum Connection {
    #[default]
    Disconnected,
    Connected {},
}
