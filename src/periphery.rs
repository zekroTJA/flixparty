use crate::config::Keys;
use anyhow::Result;
use rdev::{listen, simulate, Event, EventType, Key};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use tracing::error;

pub struct PeripheryHandler {
    keys: Keys,
}

impl PeripheryHandler {
    pub fn new(keys: Keys) -> Self {
        Self { keys }
    }

    pub fn listen(&self) -> Result<Receiver<()>> {
        let (sender, receiver) = mpsc::channel();

        let toggle_key = self.keys.toggle;
        thread::spawn(move || {
            let sender = sender.clone();
            let cb = move |e: Event| {
                if matches!(e.event_type, EventType::KeyPress(key) if key == toggle_key) {
                    if let Err(err) = sender.send(()) {
                        error!("Failed sending keypress event to mpsc channel: {err}")
                    }
                }
            };
            listen(cb).expect("listen hook");
        });

        Ok(receiver)
    }

    pub fn simulate_playback_press(&self) -> Result<()> {
        // If no key is defined in the config, MediaPalyPause key is used.
        simulate(&EventType::KeyPress(self.keys.playback))?;
        simulate(&EventType::KeyRelease(self.keys.playback))?;
        Ok(())
    }
}
