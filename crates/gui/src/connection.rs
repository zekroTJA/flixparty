//! Owns the running [`Session`]. Kept outside of the UI state so it can be
//! shut down after the event loop has exited.

use flixparty_core::{Config, Event, Session};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread;
use std::time::Duration;

/// Session events tagged with the generation of the session that sent them,
/// so events of an old session that is still shutting down can be ignored.
pub type TaggedEvent = (u64, Event);

static EVENTS: OnceLock<UnboundedSender<TaggedEvent>> = OnceLock::new();
static CURRENT: Mutex<Option<Session>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn init() -> UnboundedReceiver<TaggedEvent> {
    let (tx, rx) = unbounded();
    EVENTS.set(tx).expect("connection initialized twice");
    rx
}

pub struct Started {
    pub generation: u64,
    pub client_id: String,
    pub name: String,
}

pub fn connect(cfg: Config) -> anyhow::Result<Started> {
    disconnect();

    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let tx = EVENTS.get().expect("connection not initialized").clone();
    let session = Session::start(cfg, move |event| {
        let _ = tx.unbounded_send((generation, event));
    })?;

    let started = Started {
        generation,
        client_id: session.client_id().to_string(),
        name: session.name().to_string(),
    };
    *CURRENT.lock().expect("session lock") = Some(session);
    Ok(started)
}

/// Stops the current session in the background.
pub fn disconnect() {
    if let Some(session) = CURRENT.lock().expect("session lock").take() {
        session.stop();
    }
}

/// Stops the current session and waits a moment for it to say goodbye to the
/// other clients.
pub fn shutdown() {
    let Some(session) = CURRENT.lock().expect("session lock").take() else {
        return;
    };
    session.stop();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        session.join();
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_secs(2));
}
