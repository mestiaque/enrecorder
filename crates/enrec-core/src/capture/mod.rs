//! Capture interfaces shared by every platform backend. The backends themselves
//! (Windows.Graphics.Capture, X11, Wayland portal + PipeWire) are built in phase 2; this module
//! fixes the types and the state machine they plug into, so the UI and the session manager can
//! be written against one interface.

pub mod state;

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
    Desktop,
    Monitor { id: String, name: String, width: u32, height: u32, x: i32, y: i32, scale: f64, primary: bool },
    Window { id: String, title: String, app: String },
    Region { monitor_id: String, x: i32, y: i32, width: u32, height: u32 },
    /// Wayland: the portal dialog lets the user pick; nothing is known before that.
    PortalChoice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRequest {
    pub source: CaptureSource,
    pub fps: u32,
    pub show_cursor: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSummary {
    pub frames: u64,
    /// Frames dropped because the encoder fell behind (bounded queue policy).
    pub dropped: u64,
    pub duration_ms: u64,
}

/// One platform capture implementation. Frames are handed to a bounded queue owned by the
/// session manager; when it is full the backend drops the frame and counts it.
pub trait CaptureBackend: Send {
    fn kind(&self) -> BackendKind;
    fn list_sources(&self) -> Result<Vec<CaptureSource>>;
    fn start(&mut self, req: &CaptureRequest) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn resume(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<CaptureSummary>;
}
