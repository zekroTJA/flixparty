mod clicker;
mod config;
mod model;

use anyhow::Result;
use clicker::Clicker;
use config::Config;
use model::{Message, Op};
use rdev::{listen, Event, EventType, Key};
use redis::{Commands, Connection};
use std::{env, str::FromStr, thread};
use tracing::{debug, info};

fn main() -> Result<()> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "flixparty.config.toml".into());

    let cfg = Config::from_file(config_path)?;

    let log_level = cfg
        .log_level
        .map(|l| tracing::Level::from_str(&l))
        .transpose()?
        .unwrap_or(tracing::Level::INFO);

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(std::io::stdout)
        .init();

    let client = redis::Client::open(cfg.connection.address)?;
    info!("Redis connection established");

    let client_id = xid::new().to_string();
    info!("Your client ID is {client_id}");

    let mut pub_con = client.get_connection()?;
    let mut sub_con = client.get_connection()?;

    let mut pubsub = sub_con.as_pubsub();
    pubsub.subscribe(&cfg.connection.channel)?;
    debug!("PUBSUB subscribed to channel {}", cfg.connection.channel);

    let msg = Message {
        sender: client_id.clone(),
        op: Op::Introduce,
    };
    let _: () = pub_con.publish(&cfg.connection.channel, msg.to_json())?;

    let mut conn = Publisher {
        conn: pub_con,
        channel: cfg.connection.channel,
        client_id: client_id.clone(),
    };

    thread::spawn(move || {
        listen(move |e| conn.callback(e)).expect("keyboard listener");
    });

    let mut clicker = Clicker::new()?;

    loop {
        let msg = pubsub.get_message()?;
        let payload: String = msg.get_payload()?;
        debug!("PUBSUB message received: {payload}");

        let msg = Message::from_json(&payload)?;
        if msg.sender == client_id {
            continue;
        }

        match msg.op {
            Op::TogglePlay => clicker.click_center()?,
            Op::Introduce => info!("New client has been connected: {}", msg.sender),
        }
    }
}

pub struct Publisher {
    conn: Connection,
    channel: String,
    client_id: String,
}

impl Publisher {
    fn callback(&mut self, event: Event) {
        if matches!(event.event_type, EventType::KeyPress(Key::Space)) {
            info!("Space event detected");

            let msg = Message {
                sender: self.client_id.clone(),
                op: model::Op::TogglePlay,
            };

            let _: () = self.conn.publish(&self.channel, msg.to_json()).unwrap();
        }
    }
}
