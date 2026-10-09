use crate::state::Globals;
use crate::widgets::{ERROR, MUTED, WARNING};
use async_io::Timer;
use freya::prelude::*;
use std::time::Duration;
use tracing::{Level, error};

const LEVELS: [Level; 4] = [Level::ERROR, Level::WARN, Level::INFO, Level::DEBUG];
/// How long the copy button shows "Copied!".
const COPIED_FEEDBACK: Duration = Duration::from_millis(1500);

#[cfg(target_os = "windows")]
const MONOSPACE: &str = "Consolas";
#[cfg(target_os = "macos")]
const MONOSPACE: &str = "Menlo";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const MONOSPACE: &str = "monospace";

/// Opens the log window, or focuses it if it's already open.
pub fn open_log_window(mut g: Globals) {
    if let Some(id) = *g.log_window.read() {
        Platform::get().focus_window(Some(id));
        return;
    }

    spawn(async move {
        let id = Platform::get()
            .launch_window(
                WindowConfig::new(move || log_window(g))
                    .with_title("flixparty - Logs")
                    .with_size(900., 500.),
            )
            .await;
        g.log_window.set(Some(id));
    });
}

fn log_window(g: Globals) -> impl IntoElement {
    use_init_theme(dark_theme);
    let mut log_window = g.log_window;
    use_drop(move || log_window.set(None));

    let mut level = use_state(|| Level::INFO);
    let mut follow = use_state(|| true);
    let mut copied = use_state(|| false);
    let mut scroll = use_scroll_controller(|| ScrollConfig {
        default_vertical_position: ScrollPosition::End,
        ..Default::default()
    });

    use_side_effect(move || {
        // Subscribe to new log lines.
        let _ = g.logs.read().len();
        if *follow.peek() {
            scroll.scroll_to(ScrollPosition::End, Direction::Vertical);
        }
    });

    let max_level = *level.read();
    let logs = g.logs.read();
    let lines = logs.iter().filter(|l| l.level <= max_level).map(|l| {
        let color = match l.level {
            Level::ERROR => ERROR,
            Level::WARN => WARNING,
            Level::INFO => (220, 220, 220),
            _ => MUTED,
        };
        label()
            .width(Size::fill())
            .text(l.text.clone())
            .font_family(MONOSPACE)
            .font_size(12.)
            .color(color)
            .into()
    });

    let level_select = Select::new()
        .selected_item(level.read().to_string())
        .children(LEVELS.iter().map(|&l| {
            MenuItem::new()
                .selected(*level.read() == l)
                .on_press(move |_| level.set(l))
                .child(l.to_string())
                .into()
        }));

    let toolbar = rect()
        .width(Size::fill())
        .horizontal()
        .content(Content::Flex)
        .spacing(10.)
        .padding((8., 12.))
        .cross_align(Alignment::center())
        .background((30, 30, 30))
        .child("Level")
        .child(level_select)
        .child(
            rect()
                .horizontal()
                .spacing(6.)
                .cross_align(Alignment::center())
                .child(Switch::new().toggled(follow).on_toggle(move |_| {
                    follow.toggle();
                }))
                .child("Follow"),
        )
        .child(rect().width(Size::flex(1.)))
        .child(
            Button::new()
                .compact()
                .on_press(move |_| {
                    let max_level = *level.peek();
                    let text = g
                        .logs
                        .peek()
                        .iter()
                        .filter(|l| l.level <= max_level)
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    match Clipboard::set(text) {
                        Ok(()) => {
                            copied.set(true);
                            spawn(async move {
                                Timer::after(COPIED_FEEDBACK).await;
                                copied.set(false);
                            });
                        }
                        Err(err) => error!("Failed copying logs to the clipboard: {err:?}"),
                    }
                })
                .child(if *copied.read() { "Copied!" } else { "Copy" }),
        )
        .child(
            Button::new()
                .compact()
                .on_press(move |_| {
                    let mut logs = g.logs;
                    logs.write().clear();
                })
                .child("Clear"),
        );

    rect()
        .expanded()
        .theme_background()
        .theme_color()
        .content(Content::Flex)
        .child(toolbar)
        .child(
            ScrollView::new_controlled(scroll)
                .width(Size::fill())
                .height(Size::flex(1.))
                .child(
                    rect()
                        .width(Size::fill())
                        .padding(8.)
                        .spacing(2.)
                        .children(lines),
                ),
        )
}
