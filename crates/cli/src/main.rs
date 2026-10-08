use anyhow::Result;
use flixparty_core::{Config, Event, Session};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::{env, process};
use tracing::debug;
use yansi::Paint;

fn main() {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            // The watcher task needs to run in the main thread or else it won't
            // receive workspace events. Therefore, the run() function needs to
            // run in a separate thread.
            std::thread::spawn(|| {
                if let Err(err) = run() {
                    println!("{} {}", "error:".red().bold(), err);
                    process::exit(1);
                }
                process::exit(0);
            });
            flixparty_core::condition::start_watcher()
        } else {
            if let Err(err) = run() {
                println!("{} {}", "error:".red().bold(), err);
                process::exit(1);
            }
        }
    }
}

fn run() -> Result<()> {
    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "flixparty.config.toml".into());

    let cfg = Config::from_file(config_path)?;

    let log_level = tracing::Level::from_str(&cfg.log_level)?;
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_writer(std::io::stdout)
        .init();

    debug!("cfg: {cfg:#?}");

    // Session already logs everything relevant; only the final error is needed.
    let error = Arc::new(Mutex::new(None));
    let session = {
        let error = error.clone();
        Session::start(cfg, move |event| {
            if let Event::Stopped { error: err } = event {
                *error.lock().expect("error lock") = err;
            }
        })?
    };
    session.join();

    match error.lock().expect("error lock").take() {
        Some(err) => Err(anyhow::anyhow!(err)),
        None => Ok(()),
    }
}
