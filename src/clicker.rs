use anyhow::Result;
use enigo::{Button, Coordinate, Direction, Enigo, Mouse, Settings};

pub struct Clicker {
    handler: Enigo,
}

impl Clicker {
    pub fn new() -> Result<Self> {
        let handler = Enigo::new(&Settings::default())?;
        Ok(Self { handler })
    }

    pub fn click_center(&mut self) -> Result<()> {
        let (width, height) = self.handler.main_display()?;
        let (curr_x, curr_y) = self.handler.location()?;
        self.handler
            .move_mouse(width / 2, height / 2, Coordinate::Abs)?;
        self.handler.button(Button::Left, Direction::Click)?;
        self.handler.move_mouse(curr_x, curr_y, Coordinate::Abs)?;
        Ok(())
    }
}
