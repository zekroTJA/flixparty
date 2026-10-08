#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod app;
mod connection;
mod log_window;
mod logging;
mod settings;
mod state;
mod widgets;

use app::App;
use freya::prelude::*;
use futures_channel::mpsc::unbounded;
use state::{Globals, pump_events, pump_keys, pump_logs};

fn main() {
    let logs = logging::init();

    // Registers the frontmost app observer. The NSRunLoop it depends on is run
    // by the GUI event loop on the main thread.
    #[cfg(target_os = "macos")]
    flixparty_core::condition::init_watcher();

    let events = connection::init();
    let globals = Globals::new(settings::load());

    // Key presses are needed for recording keys in the settings. The hook
    // callback must return quickly, so keys are only handed off here.
    let (key_tx, keys) = unbounded();
    let _key_subscription = flixparty_core::periphery::subscribe(move |key| {
        let _ = key_tx.unbounded_send(key);
    });

    launch(
        LaunchConfig::new()
            .with_future(move |_| pump_logs(logs, globals.logs))
            .with_future(move |_| pump_events(events, globals))
            .with_future(move |_| pump_keys(keys, globals.form))
            .with_window(
                WindowConfig::new_app(App { globals })
                    .with_title("flixparty")
                    .with_size(560., 760.)
                    .with_min_size(420., 480.)
                    // Also close the log window, if open.
                    .with_on_close(|mut ctx, _| {
                        ctx.exit();
                        CloseDecision::Close
                    }),
            ),
    );

    connection::shutdown();
}
