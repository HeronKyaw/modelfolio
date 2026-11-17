//! OS toasts for background fetches only.
//!
//! Works via notify-rust on Windows (WinRT), macOS (mac-notification-sys)
//! and Linux (D-Bus org.freedesktop.Notifications). If the platform cannot
//! show a notification, we log it and rely on the in-app changelog badge.

pub fn send_toast(summary: &str, body: &str) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("Modelfolio")
        .summary(summary)
        .body(body)
        .show()
        .map(|_| ())
        .map_err(|e| format!("OS toast failed: {e}"))
}
