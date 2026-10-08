use tracing::debug;
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowTextW};

fn get_foreground_window_info() -> Option<(String, String)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }

        let mut title_buf = [0u16; 512];
        GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(
            &title_buf[..title_buf.iter().position(|&c| c == 0).unwrap_or(0)],
        );

        let mut class_buf = [0u16; 256];
        GetClassNameW(hwnd, &mut class_buf);
        let class = String::from_utf16_lossy(
            &class_buf[..class_buf.iter().position(|&c| c == 0).unwrap_or(0)],
        );

        Some((title, class))
    }
}

pub fn is_browser_in_focus(
    class: Option<impl AsRef<str>>,
    title_contains: Option<impl AsRef<str>>,
) -> bool {
    let Some((fg_title, fg_class)) = get_foreground_window_info() else {
        return false;
    };

    debug!(fg_title, fg_class);

    // Configured class name matches or window title contains matches
    if class.is_some_and(|c| c.as_ref() == &fg_class)
        || title_contains.is_some_and(|t| fg_title.contains(t.as_ref()))
    {
        debug!("Configured class or title matches");
        return true;
    }

    let browser_classes = [
        "Chrome_WidgetWin_1",     // Chrome, Edge (Chromium)
        "MozillaWindowClass",     // Firefox
        "OperaWindowClass",       // Opera (older)
        "ApplicationFrameWindow", // Edge legacy (UWP shell)
    ];

    let browser_title_hints = ["Chrome", "Firefox", "Edge", "Opera", "Brave", "Vivaldi"];

    let class_match = browser_classes.iter().any(|&c| fg_class == c);
    let title_match = browser_title_hints.iter().any(|&t| fg_title.contains(t));

    debug!(title_match, class_match);

    class_match || title_match
}
