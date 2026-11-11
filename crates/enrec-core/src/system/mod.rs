//! What this computer offers for capture: OS, display server, desktop, audio server and GPU
//! render nodes. Detection is split into pure functions (tested with fake environments) and
//! the small platform readers in `linux.rs` / `windows.rs`.

pub mod display;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

use serde::Serialize;

pub use display::DisplayServer;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// "linux" | "windows" | …
    pub os: String,
    /// e.g. "Ubuntu 22.04.5 LTS" or "Windows 11 (build 22631)".
    pub os_name: String,
    pub arch: String,
    pub display_server: DisplayServer,
    /// e.g. "GNOME", "KDE"; empty when unknown.
    pub desktop: String,
    /// Linux: PipeWire socket answers. Windows: not applicable (None).
    pub pipewire: Option<bool>,
    /// Linux: PulseAudio-compatible server (pipewire-pulse or PulseAudio) answers.
    pub pulse: Option<bool>,
    /// Linux: VA-API render nodes (/dev/dri/renderD*).
    pub render_nodes: Vec<String>,
}

pub fn detect() -> SystemInfo {
    #[cfg(target_os = "linux")]
    {
        linux::detect()
    }
    #[cfg(windows)]
    {
        windows::detect()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        SystemInfo {
            os: std::env::consts::OS.into(),
            os_name: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            display_server: DisplayServer::Unknown,
            desktop: String::new(),
            pipewire: None,
            pulse: None,
            render_nodes: vec![],
        }
    }
}
