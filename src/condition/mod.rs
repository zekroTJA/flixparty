cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod windows;
        pub use windows::*;
    } else if #[cfg(target_os = "macos")] {
        mod macos;
        pub use macos::*;
    } else {
        pub fn is_browser_in_focus(
            class: Option<impl AsRef<str>>,
            title_contains: Option<impl AsRef<str>>,
        ) -> bool {
            if class.is_some() || title_contains.is_some() {
                tracing::warn!("conditions are not supported in this platform");
            }
            true
        }
    }
}
