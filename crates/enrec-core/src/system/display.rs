//! Display server detection. Wayland and X11 need different capture backends, and an X11
//! session must never be assumed to support Wayland capture (or the other way round).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayServer {
    X11,
    Wayland,
    /// Windows desktop (Windows.Graphics.Capture).
    Windows,
    /// A text console or something unknown: screen capture is not possible.
    Unknown,
}

/// Decides from the session environment (`XDG_SESSION_TYPE`, `WAYLAND_DISPLAY`, `DISPLAY`).
/// `XDG_SESSION_TYPE` wins; without it a Wayland socket means Wayland even when `DISPLAY` is
/// also set (that is XWayland, which cannot capture other Wayland windows).
pub fn detect_from(get: impl Fn(&str) -> Option<String>) -> DisplayServer {
    let set = |k: &str| get(k).is_some_and(|v| !v.trim().is_empty());
    match get("XDG_SESSION_TYPE").map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("wayland") => return DisplayServer::Wayland,
        Some("x11") => return DisplayServer::X11,
        _ => {}
    }
    if set("WAYLAND_DISPLAY") {
        DisplayServer::Wayland
    } else if set("DISPLAY") {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}

/// `XDG_CURRENT_DESKTOP` is a colon list such as `ubuntu:GNOME`; the last entry is the real one.
pub fn desktop_from(value: Option<String>) -> String {
    let v = value.unwrap_or_default();
    v.split(':').map(str::trim).rfind(|s| !s.is_empty()).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn session_type_wins() {
        assert_eq!(detect_from(env(&[("XDG_SESSION_TYPE", "x11"), ("WAYLAND_DISPLAY", "wayland-0")])), DisplayServer::X11);
        assert_eq!(detect_from(env(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":0")])), DisplayServer::Wayland);
    }

    #[test]
    fn falls_back_to_sockets() {
        assert_eq!(detect_from(env(&[("WAYLAND_DISPLAY", "wayland-0"), ("DISPLAY", ":0")])), DisplayServer::Wayland);
        assert_eq!(detect_from(env(&[("DISPLAY", ":1")])), DisplayServer::X11);
        assert_eq!(detect_from(env(&[("XDG_SESSION_TYPE", "tty")])), DisplayServer::Unknown);
        assert_eq!(detect_from(env(&[("DISPLAY", "  ")])), DisplayServer::Unknown);
    }

    #[test]
    fn desktops() {
        assert_eq!(desktop_from(Some("ubuntu:GNOME".into())), "GNOME");
        assert_eq!(desktop_from(Some("KDE".into())), "KDE");
        assert_eq!(desktop_from(None), "");
    }
}
