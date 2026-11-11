//! Windows readers for [`super::SystemInfo`].

use super::display::DisplayServer;
use super::SystemInfo;

/// The Windows build (10 vs 11, Windows.Graphics.Capture support) is read in phase 2 together
/// with the capture backend; until then only the family is reported.
fn os_name() -> String {
    "Windows".into()
}

pub fn detect() -> SystemInfo {
    SystemInfo {
        os: "windows".into(),
        os_name: os_name(),
        arch: std::env::consts::ARCH.into(),
        display_server: DisplayServer::Windows,
        desktop: String::new(),
        pipewire: None,
        pulse: None,
        render_nodes: vec![],
    }
}
