use crate::retry::Retry;
use anyhow::Result;
use config::Config;
use model::{Message, Op};
use periphery::PeripheryHandler;
use redis::{Commands, Connection, ErrorKind};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{env, thread};
use tracing::{debug, error, info};
use yansi::Paint;

mod config;
mod model;
mod periphery;
mod retry;

const MAX_RETRIES: usize = 5;

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

    let log_level = tracing::Level::from_str(&cfg.log_level)?;

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(std::io::stdout)
        .init();

    for (_, remaining) in Retry::new(MAX_RETRIES, Duration::from_secs(3)) {
        let Err(err) = connect(&cfg) else {
            continue;
        };

        let Some(redis_err) = err.downcast_ref::<redis::RedisError>() else {
            return Err(err);
        };

        if matches!(
            redis_err.kind(),
            ErrorKind::ParseError | ErrorKind::AuthenticationFailed | ErrorKind::ReadOnly
        ) {
            return Err(err);
        }

        error!("connection failed: {err}; trying to reconnect ({remaining} retries remaining) ...",);
    }

    Err(anyhow::anyhow!(
        "Connection failed after {MAX_RETRIES} consecutive retries"
    ))
}

fn connect(cfg: &Config) -> Result<()> {
    let client = redis::Client::open(cfg.connection.address.as_str())?;
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
        channel: cfg.connection.channel.clone(),
        client_id: client_id.clone(),
    };

    let ph = Arc::new(PeripheryHandler::new(cfg.keys.clone()));
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

        if let Err(err) = self.conn.publish::<_, _, ()>(&self.channel, msg.to_json()) {
            error!("Failed publishing keypress event to redis connection: {err}")
        };
    }
}
