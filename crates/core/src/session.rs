//! A flixparty session: connects to Redis, publishes toggle key presses,
//! simulates the playback key on incoming toggles and keeps track of the other
//! clients in the channel.

use crate::config::Config;
use crate::model::{Message, Op, PresenceMessage, PresenceOp, presence_channel};
use crate::periphery;
use crate::retry::Retry;
use anyhow::Result;
use redis::{Commands, ConnectionInfo, ErrorKind};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, Once};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

const MAX_RETRIES: usize = 5;
const RETRY_THRESHOLD: Duration = Duration::from_secs(3);
const RETRY_DELAY: Duration = Duration::from_secs(1);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How often blocking loops wake up to check whether the session was stopped.
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
/// Members that haven't sent a heartbeat for this long are considered gone.
const MEMBER_TIMEOUT: Duration = Duration::from_secs(35);

#[derive(Debug, Clone)]
pub enum Event {
    /// A connection attempt is starting.
    Connecting,
    /// Connected and subscribed to the channel.
    Connected,
    /// The connection failed and will be retried.
    ConnectionLost {
        error: String,
        remaining_retries: usize,
    },
    MemberJoined {
        id: String,
        name: String,
    },
    MemberLeft {
        id: String,
    },
    /// A client (possibly this one) triggered a toggle.
    Toggle {
        sender: String,
        name: Option<String>,
        is_self: bool,
        /// False if the playback key was not simulated because the focused
        /// window condition didn't match.
        applied: bool,
        /// Round trip time, only set for toggles sent by this client.
        round_trip: Option<Duration>,
    },
    /// The local toggle key was pressed, but ignored because the focused
    /// window condition didn't match.
    TriggerBlocked,
    /// The session ended. `error` is set if it ended because of an error.
    Stopped {
        error: Option<String>,
    },
}

type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

pub struct Session {
    client_id: String,
    name: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Session {
    /// Validates the config and starts the session in the background. Events
    /// are passed to `on_event` from background threads.
    pub fn start(cfg: Config, on_event: impl Fn(Event) + Send + Sync + 'static) -> Result<Self> {
        cfg.validate()?;
        install_crypto_provider();

        let ctx = Ctx {
            connection_info: cfg.connection.connection_info()?,
            client_id: xid::new().to_string(),
            name: cfg.connection.display_name(),
            presence_channel: presence_channel(&cfg.connection.channel),
            cfg,
            stop: Arc::new(AtomicBool::new(false)),
            on_event: Arc::new(on_event),
            publisher: Arc::new(Mutex::new(None)),
            last_local_trigger: Arc::new(Mutex::new(None)),
        };

        info!("Your client ID is {}", ctx.client_id);

        let client_id = ctx.client_id.clone();
        let name = ctx.name.clone();
        let stop = ctx.stop.clone();
        let thread = thread::spawn(move || ctx.run());

        Ok(Self {
            client_id,
            name,
            stop,
            thread: Some(thread),
        })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Asks the session to disconnect. Returns immediately; the session
    /// finishes in the background and sends [`Event::Stopped`].
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Blocks until the session has ended.
    pub fn join(mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn install_crypto_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Fails only if another provider has already been installed.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

struct Member {
    name: String,
    last_seen: Instant,
}

struct Ctx {
    cfg: Config,
    connection_info: ConnectionInfo,
    client_id: String,
    name: String,
    presence_channel: String,
    stop: Arc<AtomicBool>,
    on_event: EventSink,
    /// The publishing connection, if currently connected.
    publisher: Arc<Mutex<Option<redis::Connection>>>,
    last_local_trigger: Arc<Mutex<Option<Instant>>>,
}

impl Ctx {
    fn emit(&self, event: Event) {
        (self.on_event)(event);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    fn run(self) {
        let trigger_thread = self.spawn_trigger_thread();
        let error = self.run_with_retries().err().map(|err| {
            error!("{err}");
            err.to_string()
        });

        self.stop.store(true, Ordering::Relaxed);
        let _ = trigger_thread.join();

        info!("Disconnected");
        self.emit(Event::Stopped { error });
    }

    fn run_with_retries(&self) -> Result<()> {
        let mut members = HashMap::new();

        for (_, remaining) in Retry::new(MAX_RETRIES, RETRY_THRESHOLD) {
            self.emit(Event::Connecting);

            let res = self.connect(&mut members);
            *self.publisher.lock().expect("publisher lock") = None;

            let Err(err) = res else {
                return Ok(());
            };

            if let Some(redis_err) = err.downcast_ref::<redis::RedisError>()
                && matches!(
                    redis_err.kind(),
                    ErrorKind::Parse
                        | ErrorKind::AuthenticationFailed
                        | ErrorKind::Client
                        | ErrorKind::InvalidClientConfig
                        | ErrorKind::Extension
                )
            {
                return Err(err);
            }

            error!(
                "connection failed: {err}; trying to reconnect ({remaining} retries remaining) ..."
            );
            self.emit(Event::ConnectionLost {
                error: err.to_string(),
                remaining_retries: remaining,
            });

            let retry_at = Instant::now() + RETRY_DELAY;
            while Instant::now() < retry_at {
                if self.stopped() {
                    return Ok(());
                }
                thread::sleep(POLL_INTERVAL);
            }
        }

        anyhow::bail!("Connection failed after {MAX_RETRIES} consecutive retries")
    }

    /// Returns `Ok` when the session was stopped.
    fn connect(&self, members: &mut HashMap<String, Member>) -> Result<()> {
        let channel = &self.cfg.connection.channel;

        let client = redis::Client::open(self.connection_info.clone())?;
        let mut pub_con = client.get_connection_with_timeout(CONNECT_TIMEOUT)?;
        let mut sub_con = client.get_connection_with_timeout(CONNECT_TIMEOUT)?;
        info!("Redis connection established");

        let mut pubsub = sub_con.as_pubsub();
        pubsub.set_read_timeout(Some(POLL_INTERVAL))?;
        pubsub.subscribe(channel)?;
        pubsub.subscribe(&self.presence_channel)?;
        debug!(
            "PUBSUB subscribed to channels {channel}, {}",
            self.presence_channel
        );

        // Introduce is kept for older clients, which only log it.
        let _: () = pub_con.publish(channel, self.message(Op::Introduce).to_json())?;
        let _: () = pub_con.publish(&self.presence_channel, self.presence(PresenceOp::Hello))?;

        *self.publisher.lock().expect("publisher lock") = Some(pub_con);
        self.emit(Event::Connected);

        let mut last_heartbeat = Instant::now();

        loop {
            if self.stopped() {
                self.publish(&self.presence_channel, self.presence(PresenceOp::Bye))?;
                return Ok(());
            }

            if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
                last_heartbeat = Instant::now();
                self.publish(&self.presence_channel, self.presence(PresenceOp::Here))?;
                self.prune_members(members);
            }

            let msg = match pubsub.get_message() {
                Ok(msg) => msg,
                Err(err) if err.is_timeout() => continue,
                Err(err) => return Err(err.into()),
            };

            let payload: String = msg.get_payload()?;
            debug!(
                "PUBSUB message received on {}: {payload}",
                msg.get_channel_name()
            );

            if msg.get_channel_name() == self.presence_channel {
                self.handle_presence(&payload, members)?;
            } else {
                self.handle_message(&payload, members)?;
            }
        }
    }

    fn handle_message(&self, payload: &str, members: &HashMap<String, Member>) -> Result<()> {
        let msg = match Message::from_json(payload) {
            Ok(msg) => msg,
            Err(err) => {
                warn!("ignoring invalid message: {err}");
                return Ok(());
            }
        };

        match msg.op {
            Op::TogglePlay => {
                let is_self = msg.sender == self.client_id;
                let round_trip = is_self
                    .then(|| *self.last_local_trigger.lock().expect("trigger lock"))
                    .flatten()
                    .map(|t| t.elapsed());
                if let Some(rtt) = round_trip {
                    debug!("Trigger round trip time: {}ms", rtt.as_millis());
                }

                let name = msg
                    .name
                    .or_else(|| members.get(&msg.sender).map(|m| m.name.clone()));
                info!(
                    "Toggle triggered by {}",
                    name.as_deref().unwrap_or(&msg.sender)
                );

                let applied = !self.cfg.condition.block_receive || self.cfg.condition.matches();
                if applied {
                    periphery::simulate_press(self.cfg.keys.playback)?;
                } else {
                    warn!(
                        "ignoring external play command because focussed window condition does not match"
                    );
                }

                self.emit(Event::Toggle {
                    sender: msg.sender,
                    name,
                    is_self,
                    applied,
                    round_trip,
                });
            }
            Op::Introduce => info!("New client has been connected: {}", msg.sender),
        }

        Ok(())
    }

    fn handle_presence(&self, payload: &str, members: &mut HashMap<String, Member>) -> Result<()> {
        let msg = match PresenceMessage::from_json(payload) {
            Ok(msg) => msg,
            Err(err) => {
                warn!("ignoring invalid presence message: {err}");
                return Ok(());
            }
        };

        if msg.sender == self.client_id {
            return Ok(());
        }

        match msg.op {
            PresenceOp::Hello | PresenceOp::Here => {
                if msg.op == PresenceOp::Hello {
                    self.publish(&self.presence_channel, self.presence(PresenceOp::Here))?;
                }

                let member = Member {
                    name: msg.name.clone(),
                    last_seen: Instant::now(),
                };
                let prev = members.insert(msg.sender.clone(), member);
                if prev.is_none_or(|prev| prev.name != msg.name) {
                    info!("{} ({}) is in the channel", msg.name, msg.sender);
                    self.emit(Event::MemberJoined {
                        id: msg.sender,
                        name: msg.name,
                    });
                }
            }
            PresenceOp::Bye => {
                if members.remove(&msg.sender).is_some() {
                    info!("{} ({}) left the channel", msg.name, msg.sender);
                    self.emit(Event::MemberLeft { id: msg.sender });
                }
            }
        }

        Ok(())
    }

    fn prune_members(&self, members: &mut HashMap<String, Member>) {
        members.retain(|id, member| {
            let alive = member.last_seen.elapsed() < MEMBER_TIMEOUT;
            if !alive {
                info!("{} ({id}) timed out", member.name);
                self.emit(Event::MemberLeft { id: id.clone() });
            }
            alive
        });
    }

    fn message(&self, op: Op) -> Message {
        Message {
            sender: self.client_id.clone(),
            op,
            name: Some(self.name.clone()),
        }
    }

    fn presence(&self, op: PresenceOp) -> String {
        PresenceMessage {
            sender: self.client_id.clone(),
            name: self.name.clone(),
            op,
        }
        .to_json()
    }

    fn publish(&self, channel: &str, payload: String) -> Result<()> {
        publish(&self.publisher, channel, payload)
    }

    /// Listens for toggle key presses and publishes them.
    fn spawn_trigger_thread(&self) -> JoinHandle<()> {
        let (tx, rx) = mpsc::channel();
        let toggle_key = self.cfg.keys.toggle;

        let condition = self.cfg.condition.clone();
        let stop = self.stop.clone();
        let on_event = self.on_event.clone();
        let publisher = self.publisher.clone();
        let last_local_trigger = self.last_local_trigger.clone();
        let channel = self.cfg.connection.channel.clone();
        let payload = self.message(Op::TogglePlay).to_json();

        thread::spawn(move || {
            let _subscription = periphery::subscribe(move |key| {
                if key == toggle_key {
                    let _ = tx.send(());
                }
            });

            while !stop.load(Ordering::Relaxed) {
                match rx.recv_timeout(POLL_INTERVAL) {
                    Ok(()) => {}
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                info!("Toggle key press event detected");

                if condition.block_trigger && !condition.matches() {
                    warn!(
                        "ignoring client play command because focussed window condition does not match"
                    );
                    on_event(Event::TriggerBlocked);
                    continue;
                }

                *last_local_trigger.lock().expect("trigger lock") = Some(Instant::now());
                if let Err(err) = publish(&publisher, &channel, payload.clone()) {
                    error!("Failed publishing keypress event to redis connection: {err}")
                }
            }
        })
    }
}

fn publish(
    publisher: &Mutex<Option<redis::Connection>>,
    channel: &str,
    payload: String,
) -> Result<()> {
    let mut publisher = publisher.lock().expect("publisher lock");
    let Some(conn) = publisher.as_mut() else {
        anyhow::bail!("not connected");
    };
    let _: () = conn.publish(channel, payload)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::mpsc::Receiver;

    fn start(name: &str) -> (Session, Receiver<Event>) {
        let mut cfg = Config::default();
        cfg.connection.address = "127.0.0.1:16379".into();
        cfg.connection.password = Some("foobar".into());
        cfg.connection.channel = "flixparty-test".into();
        cfg.connection.name = Some(name.into());

        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let session = Session::start(cfg, move |e| {
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        (session, rx)
    }

    fn wait_for(rx: &Receiver<Event>, f: impl Fn(&Event) -> bool) -> Event {
        loop {
            let event = rx.recv_timeout(Duration::from_secs(5)).expect("event");
            if f(&event) {
                return event;
            }
        }
    }

    /// Requires the Redis instance from docker-compose.yml.
    #[test]
    #[ignore]
    fn presence() {
        let (alice, alice_rx) = start("alice");
        wait_for(&alice_rx, |e| matches!(e, Event::Connected));

        let (bob, bob_rx) = start("bob");
        let Event::MemberJoined { name, .. } =
            wait_for(&bob_rx, |e| matches!(e, Event::MemberJoined { .. }))
        else {
            unreachable!()
        };
        assert_eq!(name, "alice");

        let Event::MemberJoined { id, name } =
            wait_for(&alice_rx, |e| matches!(e, Event::MemberJoined { .. }))
        else {
            unreachable!()
        };
        assert_eq!(name, "bob");
        assert_eq!(id, bob.client_id());

        bob.stop();
        wait_for(&bob_rx, |e| matches!(e, Event::Stopped { error: None }));
        let Event::MemberLeft { id } =
            wait_for(&alice_rx, |e| matches!(e, Event::MemberLeft { .. }))
        else {
            unreachable!()
        };
        assert_eq!(id, bob.client_id());

        alice.stop();
        wait_for(&alice_rx, |e| matches!(e, Event::Stopped { error: None }));
    }

    #[test]
    #[ignore]
    fn auth_failure_is_fatal() {
        let mut cfg = Config::default();
        cfg.connection.address = "127.0.0.1:16379".into();
        cfg.connection.password = Some("wrong".into());
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let _session = Session::start(cfg, move |e| {
            let _ = tx.lock().unwrap().send(e);
        })
        .unwrap();
        let event = wait_for(&rx, |e| matches!(e, Event::Stopped { .. }));
        assert!(matches!(event, Event::Stopped { error: Some(_) }));
    }
}
