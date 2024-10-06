use anyhow::Result;
use rdev::{simulate, EventType, Key};

pub struct Clicker {}

impl Clicker {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn click_center(&mut self) -> Result<()> {
        simulate(&EventType::KeyPress(Key::Space))?;
        Ok(())
    }
}
