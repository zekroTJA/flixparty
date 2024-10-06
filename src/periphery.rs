use anyhow::Result;
use rdev::{display_size, listen, simulate, Button, Event, EventType, Key};
use std::{
    sync::{mpsc, RwLock},
    thread,
};

pub struct Clicker {}

impl Clicker {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn click_center(&mut self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct PeripheryHandler {
    last_cursor_pos: RwLock<(f64, f64)>,
}

enum ListenerMessage {
    MouseEvent((f64, f64)),
    KeyEvent(Key),
}

impl PeripheryHandler {
    pub fn listen<C>(&self, mut callback: C) -> Result<()>
    where
        C: FnMut(Key),
    {
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let sender = sender.clone();
            let cb = move |e: Event| match e.event_type {
                EventType::KeyPress(key) => sender
                    .send(ListenerMessage::KeyEvent(key))
                    .expect("channel send"),
                EventType::MouseMove { x, y } => sender
                    .send(ListenerMessage::MouseEvent((x, y)))
                    .expect("channel send"),
                _ => {}
            };
            listen(cb).expect("listen hook");
        });

        loop {
            match receiver.recv()? {
                ListenerMessage::MouseEvent(pos) => *self.last_cursor_pos.write().unwrap() = pos,
                ListenerMessage::KeyEvent(key) => callback(key),
            }
        }
    }

    pub fn simulate(&self) -> Result<()> {
        let (width, height) =
            display_size().map_err(|_| anyhow::anyhow!("failed getting display size"))?;

        let (prev_x, prev_y) = *self.last_cursor_pos.read().unwrap();

        simulate(&EventType::MouseMove {
            x: (width / 2) as f64,
            y: (height / 2) as f64,
        })?;

        simulate(&EventType::ButtonPress(Button::Left))?;
        simulate(&EventType::ButtonRelease(Button::Left))?;

        simulate(&EventType::MouseMove {
            x: prev_x,
            y: prev_y,
        })?;

        Ok(())
    }
}
