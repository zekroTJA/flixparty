use anyhow::Result;
use rdev::{listen, simulate, Event, EventType};
use std::{sync::mpsc, thread};

use crate::config::Keys;

pub struct PeripheryHandler {
    keys: Keys,
}

impl PeripheryHandler {
    pub fn new(keys: Keys) -> Self {
        Self { keys }
    }

    pub fn listen<C>(&self, mut callback: C) -> Result<()>
    where
        C: FnMut(),
    {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let sender = sender.clone();
            let cb = move |e: Event| {
                if let EventType::KeyPress(key) = e.event_type {
                    sender.send(key).expect("channel send")
                }
            };
            listen(cb).expect("listen hook");
        });

        loop {
            if receiver.recv()? == self.keys.toggle {
                callback()
            }
        }
    }

    pub fn simulate(&self) -> Result<()> {
        simulate(&EventType::KeyPress(self.keys.playback))?;
        simulate(&EventType::KeyRelease(self.keys.playback))?;
        Ok(())
    }
}
