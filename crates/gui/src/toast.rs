//! Shows `Globals::error` as a toast that slides in from the bottom of the
//! window, so it's visible regardless of the scroll position.

use crate::state::Globals;
use async_io::Timer;
use freya::animation::*;
use freya::prelude::*;
use std::time::Duration;

const AUTO_HIDE: Duration = Duration::from_secs(6);
const MARGIN: f32 = 16.;
/// Offset of the toast when hidden; enough to move it out of the window.
const HIDDEN: f32 = -120.;

pub struct ErrorToast {
    pub globals: Globals,
}

// The globals never change, so there's no reason to re-render on prop changes.
impl PartialEq for ErrorToast {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Component for ErrorToast {
    fn render(&self) -> impl IntoElement {
        let mut error = self.globals.error;
        // Keeps the text while the toast slides out after the error was cleared.
        let mut text = use_state(|| None::<String>);
        let mut auto_hide = use_state(|| None::<TaskHandle>);
        let mut animation = use_animation(|_| {
            AnimNum::new(HIDDEN, MARGIN)
                .function(Function::Expo)
                .ease(Ease::Out)
                .time(300)
        });

        use_side_effect(move || {
            if let Some(task) = auto_hide.peek().as_ref() {
                task.cancel();
            }

            match error.read().clone() {
                Some(message) => {
                    text.set(Some(message));
                    animation.start();
                    auto_hide.set(Some(spawn(async move {
                        Timer::after(AUTO_HIDE).await;
                        error.set(None);
                    })));
                }
                None => {
                    auto_hide.set(None);
                    if *animation.has_run_yet().peek() {
                        animation.reverse();
                    }
                }
            }
        });

        let Some(message) = text.read().clone() else {
            return rect();
        };
        let bottom = animation.read().value();

        rect()
            .position(Position::new_absolute().bottom(bottom).left(0.))
            .layer(Layer::Overlay)
            .width(Size::fill())
            .padding((0., MARGIN))
            .child(
                rect()
                    .width(Size::fill())
                    .horizontal()
                    .content(Content::Flex)
                    .spacing(8.)
                    .padding((8., 8., 8., 14.))
                    .cross_align(Alignment::center())
                    .corner_radius(8.)
                    .background((110, 35, 35))
                    .color((255, 225, 225))
                    .shadow((0., 4., 16., 0., (0, 0, 0, 120)))
                    .child(label().width(Size::flex(1.)).text(message))
                    .child(
                        Button::new()
                            .compact()
                            .flat()
                            .on_press(move |_| error.set(None))
                            .child(label().text("×").font_size(18.)),
                    ),
            )
    }
}
