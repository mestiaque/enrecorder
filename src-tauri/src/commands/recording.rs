//! Video recording commands, the observer that turns session snapshots into UI events, and
//! the window/tray side effects (hide the main window, floating controls, tray indicator).

use super::{blocking, tauri_err};
use crate::state::AppState;
use enrec_core::capture::{BackendKind, CaptureRequest, CaptureSource};
use enrec_core::capture::state::RecState;
use enrec_core::recording::session::{self, Command, Observer, Snapshot};
use enrec_core::recording::{plan, RecordingOptions};
use enrec_core::settings::check_output_folder;
use enrec_core::{AppError, Result};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const EV_STATE: &str = "recording-state";
pub const EV_PROGRESS: &str = "recording-progress";
pub const EV_REGION: &str = "region-selected";
/// Ask the main window to start a recording with its current form (shortcut, tray).
pub const EV_START_REQUEST: &str = "recording-start-request";

/// Stop recording when less than this is free.
const MIN_FREE: u64 = 500 * 1024 * 1024;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesPayload {
    backend: Option<BackendKind>,
    /// Why capture is not possible here (Wayland/Windows backends are still to come, …).
    unsupported: Option<String>,
    sources: Vec<CaptureSource>,
}

#[tauri::command]
pub async fn list_capture_sources(app: AppHandle) -> Result<SourcesPayload> {
    blocking(move || {
        let state = app.state::<AppState>();
        match &state.capture {
            Ok(b) => Ok(SourcesPayload { backend: Some(b.kind()), unsupported: None, sources: b.list_sources()? }),
            Err(why) => Ok(SourcesPayload { backend: None, unsupported: Some(why.clone()), sources: vec![] }),
        }
    })
    .await
}

#[tauri::command]
pub fn recording_status(state: tauri::State<AppState>) -> Option<Snapshot> {
    state.recorder.lock().as_ref().map(|h| h.snapshot())
}

/// The file name for a new recording: "Recording 2026-11-11 10-02-33".
fn file_stem() -> String {
    let now = tauri_plugin_log::TimezoneStrategy::UseLocal.get_now();
    let f = time::macros::format_description!("[year]-[month]-[day] [hour]-[minute]-[second]");
    format!("Recording {}", now.format(&f).unwrap_or_default())
}

#[tauri::command]
pub async fn start_video_recording(app: AppHandle, options: RecordingOptions) -> Result<Snapshot> {
    blocking(move || {
        let state = app.state::<AppState>();
        if state.recording_active() {
            return Err(AppError::Other("A recording is already running".into()));
        }
        let backend = state.capture.as_ref().map_err(|why| AppError::Unsupported(why.clone()))?.clone();
        let tools = state.tools.read().clone();
        let ffmpeg = tools.ffmpeg.ok_or(AppError::FfmpegMissing)?;
        let caps = tools.capabilities.ok_or_else(|| AppError::Other("The encoders are still being tested; try again in a few seconds".into()))?;
        let output_dir = state.recordings_folder();
        check_output_folder(&output_dir.to_string_lossy())?;
        let input = backend.input(&CaptureRequest { source: options.source.clone(), fps: options.fps, show_cursor: options.show_cursor })?;
        let plan = plan(&options, &input, &caps)?;
        let observer = Arc::new(UiObserver { app: app.clone(), last_progress: Mutex::new(Instant::now() - Duration::from_secs(1)) });
        let handle = session::start(
            session::StartParams {
                db: state.db.clone(),
                ffmpeg: PathBuf::from(&ffmpeg.path),
                ffprobe: tools.ffprobe.map(|t| PathBuf::from(t.path)),
                input,
                plan,
                options,
                output_dir,
                file_stem: file_stem(),
                min_free_bytes: MIN_FREE,
            },
            observer,
        )?;
        let snap = handle.snapshot();
        *state.recorder.lock() = Some(handle);
        Ok(snap)
    })
    .await
}

/// Microphones and system-audio sources (PulseAudio/PipeWire on Linux, DirectShow on Windows).
#[tauri::command]
pub async fn list_audio_devices(app: AppHandle) -> Result<Vec<enrec_core::audio::AudioDevice>> {
    blocking(move || {
        let ffmpeg = app.state::<AppState>().tools.read().ffmpeg.clone().map(|t| PathBuf::from(t.path));
        enrec_core::audio::list_devices(ffmpeg.as_deref())
    })
    .await
}

/// "Audio 2026-11-11 10-02-33".
fn audio_stem() -> String {
    file_stem().replacen("Recording", "Audio", 1)
}

/// Audio Recorder: microphone and/or system audio to MP3/M4A/Opus/FLAC/WAV, same session
/// engine as video (segments on pause, verified file, library row).
#[tauri::command]
pub async fn start_audio_recording(app: AppHandle, options: enrec_core::recording::AudioRecordingOptions) -> Result<Snapshot> {
    blocking(move || {
        let state = app.state::<AppState>();
        if state.recording_active() {
            return Err(AppError::Other("A recording is already running".into()));
        }
        let tools = state.tools.read().clone();
        let ffmpeg = tools.ffmpeg.ok_or(AppError::FfmpegMissing)?;
        let output_dir = state.audio_folder();
        check_output_folder(&output_dir.to_string_lossy())?;
        let plan = enrec_core::recording::plan_audio(&options)?;
        // The session needs video-shaped options for its timing fields; the source is unused.
        let rec_opts = RecordingOptions {
            source: CaptureSource::Desktop { width: 0, height: 0 },
            fps: 30,
            resolution: "native".into(),
            format: options.format.clone(),
            codec: enrec_core::media::ffmpeg::Codec::H264,
            encoder: plan.encoder.clone(),
            quality: options.quality.clone(),
            show_cursor: false,
            countdown: options.countdown.min(10),
            max_minutes: options.max_minutes,
            audio: options.audio.clone(),
        };
        let observer = Arc::new(UiObserver { app: app.clone(), last_progress: Mutex::new(Instant::now() - Duration::from_secs(1)) });
        let handle = session::start(
            session::StartParams {
                db: state.db.clone(),
                ffmpeg: PathBuf::from(&ffmpeg.path),
                ffprobe: tools.ffprobe.map(|t| PathBuf::from(t.path)),
                input: enrec_core::capture::CaptureInput { args: vec![], width: 0, height: 0, frames: None },
                plan,
                options: rec_opts,
                output_dir,
                file_stem: audio_stem(),
                min_free_bytes: MIN_FREE,
            },
            observer,
        )?;
        let snap = handle.snapshot();
        *state.recorder.lock() = Some(handle);
        Ok(snap)
    })
    .await
}

fn send(state: &AppState, cmd: Command) -> Result<()> {
    match state.recorder.lock().as_ref() {
        Some(h) if h.is_active() => h.send(cmd),
        _ => Err(AppError::Other("No recording is running".into())),
    }
}

#[tauri::command]
pub fn pause_recording(state: tauri::State<AppState>) -> Result<()> {
    send(&state, Command::Pause)
}

#[tauri::command]
pub fn resume_recording(state: tauri::State<AppState>) -> Result<()> {
    send(&state, Command::Resume)
}

#[tauri::command]
pub fn stop_recording(state: tauri::State<AppState>) -> Result<()> {
    let st = state.recorder.lock().as_ref().map(|h| h.snapshot().state);
    // Stop during the countdown means "do not record".
    send(&state, if matches!(st, Some(RecState::Countdown | RecState::Preparing)) { Command::Cancel } else { Command::Stop })
}

/// Pause ↔ resume (shortcut and tray).
pub fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let st = state.recorder.lock().as_ref().map(|h| h.snapshot().state);
    let _ = match st {
        Some(RecState::Recording) => send(&state, Command::Pause),
        Some(RecState::Paused) => send(&state, Command::Resume),
        _ => Ok(()),
    };
}

/// Start ↔ stop (shortcut and tray). Starting needs the form, so the main window does it.
pub fn toggle_record(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.recording_active() {
        if let Err(e) = stop_recording(app.state::<AppState>()) {
            log::warn!("stop: {e}");
        }
    } else {
        log::info!("start requested (shortcut or tray)");
        let _ = app.emit_to("main", EV_START_REQUEST, ());
    }
}

/// Stops a running recording and waits until the file is finished (used before quitting).
pub fn stop_and_wait(app: &AppHandle, timeout: Duration) {
    let state = app.state::<AppState>();
    let mut guard = state.recorder.lock();
    if let Some(h) = guard.as_mut() {
        if h.is_active() {
            let st = h.snapshot().state;
            let _ = h.send(if matches!(st, RecState::Countdown | RecState::Preparing) { Command::Cancel } else { Command::Stop });
            if !h.wait(timeout) {
                log::warn!("the recording did not finish within {} s", timeout.as_secs());
            }
        }
    }
}

struct UiObserver {
    app: AppHandle,
    last_progress: Mutex<Instant>,
}

impl Observer for UiObserver {
    fn state(&self, s: &Snapshot) {
        let _ = self.app.emit(EV_STATE, s);
        let app = self.app.clone();
        let s = s.clone();
        // Window and tray changes must happen on the main thread.
        let _ = self.app.run_on_main_thread(move || on_state(&app, &s));
    }

    fn progress(&self, s: &Snapshot) {
        // The UI needs a few updates per second; capture keeps full precision internally.
        let mut last = self.last_progress.lock();
        if last.elapsed() >= Duration::from_millis(250) {
            *last = Instant::now();
            let _ = self.app.emit(EV_PROGRESS, s);
        }
    }
}

fn on_state(app: &AppHandle, s: &Snapshot) {
    let state = app.state::<AppState>();
    let (hide, hud) = {
        let st = state.settings.read();
        (st.rec_hide_window, st.rec_show_hud)
    };
    crate::tray::show_recording(app, s.state);
    // The Audio Recorder stays visible: its level meter is the feedback.
    let (hide, hud) = if s.audio_only { (false, false) } else { (hide, hud) };
    match s.state {
        RecState::Recording if s.segments <= 1 => {
            if hud {
                open_hud(app);
            }
            if hide {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
        }
        st if st.is_final() => {
            if let Some(w) = app.get_webview_window("hud") {
                let _ = w.close();
            }
            crate::tray::show_main(app);
        }
        _ => {}
    }
}

/// The floating controls: a small always-on-top bar at the top centre of the primary monitor.
fn open_hud(app: &AppHandle) {
    if app.get_webview_window("hud").is_some() {
        return;
    }
    let (w, h) = (300.0, 52.0);
    let mut b = WebviewWindowBuilder::new(app, "hud", WebviewUrl::App("index.html?view=hud".into()))
        .title("ENRecorder controls")
        .inner_size(w, h)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .shadow(false);
    if let Some(m) = app.primary_monitor().ok().flatten() {
        let scale = m.scale_factor();
        let size = m.size().to_logical::<f64>(scale);
        let pos = m.position().to_logical::<f64>(scale);
        b = b.position(pos.x + (size.width - w) / 2.0, pos.y + 12.0);
    }
    if let Err(e) = b.build() {
        log::warn!("could not open the floating controls: {e}");
    }
}

/// Opens a transparent full-screen overlay on the monitor under the main window to draw the
/// area to record. The result comes back through [`region_selected`].
#[tauri::command]
pub fn open_region_selector(app: AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window("region") {
        let _ = w.set_focus();
        return Ok(());
    }
    let main = app.get_webview_window("main");
    let monitor = main.as_ref().and_then(|w| w.current_monitor().ok().flatten()).or_else(|| app.primary_monitor().ok().flatten()).ok_or_else(|| AppError::Unsupported("no monitor found".into()))?;
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let pos = monitor.position();
    let url = format!("index.html?view=region&ox={}&oy={}&scale={scale}", pos.x, pos.y);
    let lpos = pos.to_logical::<f64>(scale);
    WebviewWindowBuilder::new(&app, "region", WebviewUrl::App(url.into()))
        .title("Select the area to record")
        .position(lpos.x, lpos.y)
        .inner_size(size.width, size.height)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(true)
        .build()
        .map_err(tauri_err)?;
    if let Some(m) = main {
        let _ = m.hide();
    }
    Ok(())
}

/// Called by the overlay: `rect` in physical desktop pixels, or `None` when cancelled.
#[tauri::command]
pub fn region_selected(app: AppHandle, rect: Option<CaptureSource>) -> Result<()> {
    if let Some(r) = &rect {
        if !matches!(r, CaptureSource::Region { width, height, .. } if *width >= 16 && *height >= 16) {
            return Err(AppError::InvalidSetting("the area is too small".into()));
        }
    }
    let _ = app.emit_to("main", EV_REGION, rect);
    if let Some(w) = app.get_webview_window("region") {
        let _ = w.close();
    }
    crate::tray::show_main(&app);
    Ok(())
}
