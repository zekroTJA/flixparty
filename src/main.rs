mod config;
mod model;
mod periphery;

use anyhow::Result;
use config::Config;
use model::{Message, Op};
use periphery::PeripheryHandler;
use redis::{Commands, Connection};
use std::{
    env,
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::SystemTime,
};
use tracing::{debug, info};
use yansi::Paint;

fn main() {
    if let Err(err) = run() {
        println!("{} {}", "error:".red().bold(), err);
    }
}

fn run() -> Result<()> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "flixparty.config.toml".into());

    let cfg = Config::from_file(config_path)?;

    if cfg.keys.playback == cfg.keys.toggle {
        anyhow::bail!("playback and toggle key must not be the same key")
    }

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

    let mut publisher = Publisher {
        conn: pub_con,
        channel: cfg.connection.channel,
        client_id: client_id.clone(),
    };

    let ph = Arc::new(PeripheryHandler::new(cfg.keys));
    let last_local_trigger = Arc::new(Mutex::new(None));

    {
        let ph = ph.clone();
        let last_local_trigger = last_local_trigger.clone();
        thread::spawn(move || {
            let rec = ph.listen().expect("keyboard listener");
            loop {
                rec.recv().expect("channel receive");
                *last_local_trigger.lock().expect("acquire lock") = Some(SystemTime::now());
                publisher.broadcast_toggle();
            }
        });
    }

    loop {
        let msg = pubsub.get_message()?;
        let payload: String = msg.get_payload()?;
        debug!("PUBSUB message received: {payload}");

        let msg = Message::from_json(&payload)?;

        match msg.op {
            Op::TogglePlay => {
                if msg.sender == client_id {
                    let Some(v) = *last_local_trigger.lock().expect("mutex lock") else {
                        panic!("last_local_trigger was None on self-sent event - this should not happen");
                    };
                    let now = SystemTime::now().duration_since(v)?;
                    debug!("Trigger round trip time: {}ms", now.as_millis());
                }
                ph.simulate_playback_press()?;
            }
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
    fn broadcast_toggle(&mut self) {
        info!("Toggle key press event detected");

        let msg = Message {
            sender: self.client_id.clone(),
            op: model::Op::TogglePlay,
        };

        let _: () = self.conn.publish(&self.channel, msg.to_json()).unwrap();
    }
}
