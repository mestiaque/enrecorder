//! Capture sources and platform backends. A backend lists what can be captured and turns a
//! request into the FFmpeg input that captures it; the recording session (see
//! [`crate::recording`]) owns encoding, pause/resume (segments), timing and finalisation, so
//! every backend gets the same controls.
//!
//! Backends: X11 (FFmpeg's XCB/XShm grabber, sources from EWMH + RandR) — done.
//! Wayland (xdg-desktop-portal ScreenCast + PipeWire) and Windows (Windows.Graphics.Capture)
//! are planned for the rest of phase 2.

pub mod frames;
pub mod state;
#[cfg(all(target_os = "linux", feature = "wayland"))]
pub mod wayland;
#[cfg(target_os = "linux")]
pub mod x11;

use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackendKind {
    WindowsGraphicsCapture,
    X11,
    WaylandPortal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum CaptureSource {
    /// Everything (all monitors).
    Desktop { width: u32, height: u32 },
    Monitor { id: String, name: String, width: u32, height: u32, x: i32, y: i32, primary: bool },
    Window { id: String, title: String, app: String, width: u32, height: u32 },
    /// Physical pixels in desktop coordinates.
    Region { x: i32, y: i32, width: u32, height: u32 },
    /// Wayland: the desktop's sharing dialog lets the user pick a screen or window when the
    /// recording starts; nothing can be captured without that choice.
    Portal,
}

impl CaptureSource {
    pub fn kind_label(&self) -> &'static str {
        match self {
            CaptureSource::Desktop { .. } => "desktop",
            CaptureSource::Monitor { .. } => "monitor",
            CaptureSource::Window { .. } => "window",
            CaptureSource::Region { .. } => "region",
            CaptureSource::Portal => "portal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub source: CaptureSource,
    pub fps: u32,
    pub show_cursor: bool,
}

/// What FFmpeg reads: either a device FFmpeg opens itself (`args`, including `-i …`), or
/// frames ENRecorder delivers through a pipe (`frames`, `args` empty), plus the captured size.
pub struct CaptureInput {
    pub args: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub frames: Option<Box<dyn frames::FrameSource>>,
}

impl std::fmt::Debug for CaptureInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureInput").field("args", &self.args).field("width", &self.width).field("height", &self.height).field("frames", &self.frames.is_some()).finish()
    }
}

pub trait CaptureBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    /// Desktop, monitors and windows that can be captured right now.
    fn list_sources(&self) -> Result<Vec<CaptureSource>>;
    /// The FFmpeg input for `req`; checks the source still exists.
    fn input(&self, req: &CaptureRequest) -> Result<CaptureInput>;
}

/// Largest even number ≤ `v` (H.264/HEVC 4:2:0 need even sizes), at least 2.
pub fn even(v: u32) -> u32 {
    (v & !1).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_sizes() {
        assert_eq!(even(1921), 1920);
        assert_eq!(even(1080), 1080);
        assert_eq!(even(1), 2);
    }

    #[test]
    fn sources_serialize_with_a_type_tag() {
        let v = serde_json::to_value(CaptureSource::Region { x: 1, y: 2, width: 3, height: 4 }).unwrap();
        assert_eq!(v["type"], "region");
        let back: CaptureSource = serde_json::from_value(serde_json::json!({"type": "desktop", "width": 10, "height": 20})).unwrap();
        assert_eq!(back, CaptureSource::Desktop { width: 10, height: 20 });
    }
}
