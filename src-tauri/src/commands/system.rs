use super::tauri_err;
use crate::state::AppState;
use enrec_core::system::SystemInfo;
use enrec_core::{AppError, Result};
use std::io::{Read, Seek, SeekFrom};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn frontend_log(level: String, message: String) {
    match level.as_str() {
        "error" => log::error!(target: "webview", "{message}"),
        "warn" => log::warn!(target: "webview", "{message}"),
        _ => log::info!(target: "webview", "{message}"),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    data: String,
    logs: String,
    recordings: String,
    screenshots: String,
}

#[tauri::command]
pub fn app_paths(state: State<AppState>) -> AppPaths {
    AppPaths {
        data: state.data_dir.to_string_lossy().into_owned(),
        logs: state.log_dir.to_string_lossy().into_owned(),
        recordings: state.recordings_folder().to_string_lossy().into_owned(),
        screenshots: state.screenshots_folder().to_string_lossy().into_owned(),
    }
}

/// A display as Tauri sees it (used to show what can be captured).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    name: String,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    scale: f64,
    primary: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPayload {
    #[serde(flatten)]
    info: SystemInfo,
    monitors: Vec<MonitorInfo>,
}

/// OS, display server, audio server and connected monitors. Re-detected on every call so a
/// plugged-in monitor shows up without a restart.
#[tauri::command]
pub fn system_info(window: WebviewWindow, state: State<AppState>) -> SystemPayload {
    let info = enrec_core::system::detect();
    *state.system.write() = info.clone();
    let primary = window.primary_monitor().ok().flatten().map(|m| (m.position().x, m.position().y));
    let monitors = window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            name: m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1)),
            width: m.size().width,
            height: m.size().height,
            x: m.position().x,
            y: m.position().y,
            scale: m.scale_factor(),
            primary: primary == Some((m.position().x, m.position().y)),
        })
        .collect();
    SystemPayload { info, monitors }
}

/// The last `max_lines` lines of the log file.
pub fn tail_log(state: &AppState, max_lines: usize) -> String {
    let Ok(mut f) = std::fs::File::open(state.log_file()) else { return String::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if f.seek(SeekFrom::Start(len.saturating_sub(512 * 1024))).is_err() {
        return String::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(max_lines)..].join("\n")
}

#[tauri::command]
pub fn read_logs(state: State<AppState>, max_lines: Option<usize>) -> String {
    tail_log(&state, max_lines.unwrap_or(500).min(5000))
}

/// Text for "Copy diagnostics": versions, system, tools and recent log lines.
#[tauri::command]
pub fn diagnostics(app: AppHandle, state: State<AppState>) -> Result<String> {
    let sys = state.system.read().clone();
    let tools = state.tools.read().clone();
    let stats = state.db.library_stats()?;
    let mut out = String::new();
    out.push_str(&format!("ENRecorder {} ({})\n", app.package_info().version, if cfg!(debug_assertions) { "debug" } else { "release" }));
    out.push_str(&format!("OS: {} ({})\n", sys.os_name, sys.arch));
    out.push_str(&format!("Display: {:?} · desktop {} · PipeWire {:?} · Pulse {:?}\n", sys.display_server, if sys.desktop.is_empty() { "?" } else { &sys.desktop }, sys.pipewire, sys.pulse));
    out.push_str(&format!("Render nodes: {}\n", sys.render_nodes.join(", ")));
    out.push_str(&format!("Tauri {} · WebView {}\n", tauri::VERSION, tauri::webview_version().unwrap_or_default()));
    out.push_str(&format!("FFmpeg: {}\n", tools.ffmpeg.as_ref().map_or("not found".into(), |t| format!("{} ({})", t.version, t.path))));
    out.push_str(&format!("FFprobe: {}\n", tools.ffprobe.as_ref().map_or("not found".into(), |t| format!("{} ({})", t.version, t.path))));
    if let Some(c) = &tools.capabilities {
        let ok: Vec<&str> = c.encoders.iter().filter(|e| e.verified).map(|e| e.name.as_str()).collect();
        out.push_str(&format!("Verified encoders: {}\n", if ok.is_empty() { "none".into() } else { ok.join(", ") }));
    }
    out.push_str(&format!("Database schema: {}\n", state.db.schema_version()?));
    out.push_str(&format!("Library: {} videos, {} audio, {} screenshots\n", stats.videos, stats.audio, stats.screenshots));
    out.push_str("\n--- recent log ---\n");
    out.push_str(&tail_log(&state, 200));
    Ok(out)
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<()> {
    app.opener().open_path(path, None::<&str>).map_err(tauri_err)
}

/// Opens a save folder, creating it first (it may not exist before the first capture).
#[tauri::command]
pub fn open_folder(app: AppHandle, path: String) -> Result<()> {
    let p = std::path::Path::new(&path);
    if !p.is_absolute() {
        return Err(AppError::InvalidPath(path));
    }
    std::fs::create_dir_all(p)?;
    app.opener().open_path(path, None::<&str>).map_err(tauri_err)
}

#[tauri::command]
pub fn reveal_path(app: AppHandle, path: String) -> Result<()> {
    let p = std::path::PathBuf::from(&path);
    if p.exists() {
        app.opener().reveal_item_in_dir(&p).map_err(tauri_err)
    } else if let Some(dir) = p.parent().filter(|d| d.exists()) {
        app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(tauri_err)
    } else {
        Err(AppError::NotFound(path))
    }
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err(AppError::Other(format!("refusing to open {url}")));
    }
    app.opener().open_url(url, None::<&str>).map_err(tauri_err)
}

#[tauri::command]
pub fn open_log_folder(app: AppHandle, state: State<AppState>) -> Result<()> {
    std::fs::create_dir_all(&state.log_dir)?;
    app.opener().open_path(state.log_dir.to_string_lossy(), None::<&str>).map_err(tauri_err)
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::quit(&app);
}

#[tauri::command]
pub fn hide_to_tray(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutInfo {
    pub build: String,
    pub tauri_version: String,
    pub webview_version: String,
    pub platform: String,
    pub arch: String,
}

#[tauri::command]
pub fn about_info() -> AboutInfo {
    AboutInfo {
        build: if cfg!(debug_assertions) { "debug" } else { "release" }.into(),
        tauri_version: tauri::VERSION.into(),
        webview_version: tauri::webview_version().unwrap_or_default(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
    }
}
