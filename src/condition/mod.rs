cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    } else {
        pub fn is_browser_in_focus(matcher: Option<crate::config::Condition>) -> bool {
            if matcher.is_some() {
                tracing::warn!("conditions are not supported in this platform");
            }
            true
        }
    }
}
