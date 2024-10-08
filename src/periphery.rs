use anyhow::Result;
use rdev::{listen, simulate, Event, EventType};
use std::{
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::config::Keys;

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
                    sender.send(()).expect("channel send");
                }
            };
            listen(cb).expect("listen hook");
        });

        Ok(receiver)
    }

    pub fn simulate_playback_press(&self) -> Result<()> {
        simulate(&EventType::KeyPress(self.keys.playback))?;
        simulate(&EventType::KeyRelease(self.keys.playback))?;
        Ok(())
    }
}
