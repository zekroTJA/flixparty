use frontmost::app::FrontmostApp;
use frontmost::{start_nsrunloop, Detector};
use std::sync::{Mutex, OnceLock};
use tracing::{debug, error};

// Global state to hold the current frontmost app name
static FRONTMOST_APP: OnceLock<Mutex<String>> = OnceLock::new();

fn app_state() -> &'static Mutex<String> {
    FRONTMOST_APP.get_or_init(|| Mutex::new(String::new()))
}

#[derive(Debug)]
struct AppWatcher;

impl FrontmostApp for AppWatcher {
    fn set_frontmost(&mut self, new_value: &str) {
        debug!("update focussed window: {new_value}");
        if let Ok(mut state) = app_state().lock() {
            *state = new_value.to_string();
        }
    }

    fn update(&mut self) {}
}

pub fn start_watcher() {
    Detector::init(Box::new(AppWatcher));
    debug!("window watcher initialized; starting event loop ...");
    start_nsrunloop!();
}

pub fn is_browser_in_focus(
    class: Option<impl AsRef<str>>,
    title_contains: Option<impl AsRef<str>>,
) -> bool {
    let Some(app) = app_state().lock().ok() else {
        error!("failed getting app_state mutex guard");
        return false;
    };

    if class.is_some_and(|c| c.as_ref() == app.as_str())
        || title_contains.is_some_and(|t| app.contains(t.as_ref()))
    {
        return true;
    }

    let browser_apps = [
        "Google Chrome",
        "Firefox",
        "Safari",
        "Opera",
        "Brave Browser",
        "Vivaldi",
    ];

    browser_apps.iter().any(|&b| app.contains(b))
}
