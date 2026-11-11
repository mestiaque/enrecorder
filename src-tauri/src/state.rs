use enrec_core::capture::CaptureBackend;
use enrec_core::database::Db;
use enrec_core::recording::session::SessionHandle;
use enrec_core::media::ffmpeg::{Capabilities, ToolInfo};
use enrec_core::settings::Settings;
use enrec_core::system::SystemInfo;
use parking_lot::{Mutex, RwLock};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// FFmpeg/FFprobe as found at start-up (or after the path changed in Settings).
#[derive(Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTools {
    pub ffmpeg: Option<ToolInfo>,
    pub ffprobe: Option<ToolInfo>,
    /// Verified encoder capabilities; None until the first check finished.
    pub capabilities: Option<Capabilities>,
    /// A search or capability check is running.
    pub checking: bool,
}

pub struct AppState {
    pub db: Arc<Db>,
    pub settings: RwLock<Settings>,
    pub system: RwLock<SystemInfo>,
    pub tools: RwLock<MediaTools>,
    /// Set by "Quit" so the close-to-tray handler lets the window go.
    pub quitting: AtomicBool,
    /// The capture backend for this session's display server, or why there is none.
    pub capture: Result<Arc<dyn CaptureBackend>, String>,
    /// The current (or last) recording.
    pub recorder: Mutex<Option<SessionHandle>>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub system_videos: PathBuf,
    pub system_pictures: PathBuf,
}

impl AppState {
    pub fn recordings_folder(&self) -> PathBuf {
        self.settings.read().recordings_folder(&self.system_videos)
    }

    pub fn screenshots_folder(&self) -> PathBuf {
        self.settings.read().screenshots_folder(&self.system_pictures)
    }

    pub fn log_file(&self) -> PathBuf {
        self.log_dir.join("enrecorder.log")
    }

    /// Bundled FFmpeg locations: next to the program (Windows build) and the managed tools
    /// folder in the data directory.
    pub fn bundled_tool_dirs(&self) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(PathBuf::from)) {
            v.push(dir);
        }
        v.push(self.data_dir.join("tools"));
        v
    }

    pub fn capabilities_cache(&self) -> PathBuf {
        self.data_dir.join("cache").join("encoders.json")
    }
}

impl AppState {
    /// A recording is preparing, counting down, running, paused or being finalised.
    pub fn recording_active(&self) -> bool {
        self.recorder.lock().as_ref().is_some_and(|h| h.is_active())
    }
}
