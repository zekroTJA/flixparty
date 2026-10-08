//! Global keyboard hook and key simulation.
//!
//! rdev only supports a single global listener per process (later calls to
//! `rdev::listen` replace the callback of earlier ones). So there is exactly
//! one listener thread, started on the first [`subscribe`] call, which forwards
//! every key press to all current subscribers.

use anyhow::Result;
use rdev::{EventType, Key, listen, simulate};
use std::sync::{Mutex, OnceLock};
use std::thread;
use tracing::{debug, error};

type Callback = Box<dyn Fn(Key) + Send>;

#[derive(Default)]
struct Hub {
    started: bool,
    next_id: u64,
    subscribers: Vec<(u64, Callback)>,
}

fn global_hub() -> &'static Mutex<Hub> {
    static HUB: OnceLock<Mutex<Hub>> = OnceLock::new();
    HUB.get_or_init(Default::default)
}

/// Unsubscribes the callback when dropped.
pub struct Subscription(u64);

impl Drop for Subscription {
    fn drop(&mut self) {
        let mut hub = global_hub().lock().expect("hub lock");
        hub.subscribers.retain(|(id, _)| *id != self.0);
    }
}

/// Calls `callback` for every key press until the returned [`Subscription`] is
/// dropped.
///
/// The callback runs on the hook thread. On Windows, slow hook callbacks get
/// the hook removed by the OS, so it should only hand the key off (e.g. over a
/// channel) and return.
pub fn subscribe(callback: impl Fn(Key) + Send + 'static) -> Subscription {
    let mut hub = global_hub().lock().expect("hub lock");

    if !hub.started {
        hub.started = true;
        thread::spawn(|| {
            debug!("starting global keyboard listener");
            let res = listen(|e| {
                if let EventType::KeyPress(key) = e.event_type {
                    let hub = global_hub().lock().expect("hub lock");
                    for (_, cb) in &hub.subscribers {
                        cb(key);
                    }
                }
            });
            if let Err(err) = res {
                error!("Global keyboard listener failed: {err:?}");
                global_hub().lock().expect("hub lock").started = false;
            }
        });
    }

    let id = hub.next_id;
    hub.next_id += 1;
    hub.subscribers.push((id, Box::new(callback)));
    Subscription(id)
}

pub fn simulate_press(key: Key) -> Result<()> {
    simulate(&EventType::KeyPress(key))?;
    simulate(&EventType::KeyRelease(key))?;
    Ok(())
}

/// A human readable name for a key.
pub fn key_name(key: Key) -> String {
    #[cfg(target_os = "windows")]
    if key == Key::Unknown(179) {
        return "Media Play/Pause".into();
    }
    #[cfg(target_os = "linux")]
    if key == Key::Unknown(172) {
        return "Media Play/Pause".into();
    }

    let name = format!("{key:?}");
    if let Some(rest) = name.strip_prefix("Key")
        && rest.len() == 1
    {
        return rest.into();
    }
    if let Some(rest) = name.strip_prefix("Num")
        && rest.len() == 1
    {
        return rest.into();
    }
    match key {
        Key::Unknown(code) => format!("Key code {code}"),
        _ => name,
    }
}
