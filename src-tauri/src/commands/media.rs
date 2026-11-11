//! FFmpeg status and encoder verification.

use crate::state::{AppState, MediaTools};
use enrec_core::media::ffmpeg;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

/// Sent whenever the FFmpeg status or the verified encoders change.
pub const EV_TOOLS: &str = "tools-changed";

#[tauri::command]
pub fn media_tools(state: State<AppState>) -> MediaTools {
    state.tools.read().clone()
}

/// Re-runs the encoder tests (Settings → Tools → "Test again").
#[tauri::command]
pub fn verify_encoders(app: AppHandle) {
    refresh_tools(app, true);
}

/// Finds FFmpeg/FFprobe and loads (or computes) the verified encoders in the background.
/// `force` ignores the cache. Emits [`EV_TOOLS`] when the state changes.
pub fn refresh_tools(app: AppHandle, force: bool) {
    {
        let state = app.state::<AppState>();
        let mut t = state.tools.write();
        if t.checking {
            return;
        }
        t.checking = true;
    }
    let _ = app.emit(EV_TOOLS, ());
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let custom = state.settings.read().ffmpeg_path.clone();
        let custom = (!custom.is_empty()).then(|| PathBuf::from(custom));
        let dirs = state.bundled_tool_dirs();
        let found = ffmpeg::find_tool("ffmpeg", custom.as_deref(), &dirs);
        let probe = ffmpeg::find_tool("ffprobe", custom.as_deref(), &dirs);
        match &found {
            Some(t) => log::info!("FFmpeg {} at {}", t.version, t.path),
            None => log::warn!("FFmpeg not found"),
        }
        {
            let mut t = state.tools.write();
            t.ffmpeg = found.clone();
            // Windows: which grabber this FFmpeg offers (ddagrab or gdigrab).
            #[cfg(target_os = "windows")]
            if let Some(f) = &found {
                enrec_core::capture::windows::set_ffmpeg(std::path::Path::new(&f.path));
            }
            t.ffprobe = probe;
            t.capabilities = None;
        }
        let _ = app.emit(EV_TOOLS, ());
        let caps = found.and_then(|tool| {
            let cache = state.capabilities_cache();
            if !force {
                if let Some(c) = ffmpeg::load_cached(&cache, &tool) {
                    return Some(c);
                }
            }
            match ffmpeg::check_capabilities(&tool) {
                Ok(c) => {
                    if let Err(e) = ffmpeg::save_cache(&cache, &c) {
                        log::warn!("could not cache encoder results: {e}");
                    }
                    Some(c)
                }
                Err(e) => {
                    log::error!("encoder check failed: {e}");
                    None
                }
            }
        });
        {
            let mut t = state.tools.write();
            t.capabilities = caps;
            t.checking = false;
        }
        let _ = app.emit(EV_TOOLS, ());
    });
}
