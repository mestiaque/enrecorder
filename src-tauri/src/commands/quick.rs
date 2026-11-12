//! Quick capture (Flameshot / ShareX style): a shortcut freezes the whole desktop into an image,
//! a full-screen overlay shows it, the user drags the area and annotates it in place (arrow, text,
//! blur…), then copies it (Enter), saves it to the library (Ctrl+S) or opens it in the Screenshot
//! Studio. The full capture is kept as the original and the area as the crop, so a saved quick
//! capture stays editable like any other screenshot.

use super::{blocking, tauri_err};
use crate::state::AppState;
use base64::Engine;
use enrec_core::capture::{CaptureRequest, CaptureSource};
use enrec_core::screenshot;
use enrec_core::{AppError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub const LABEL: &str = "quick";
/// The main window opens a saved quick capture in the Screenshot Studio editor.
pub const EV_OPEN_SHOT: &str = "open-screenshot";

/// The frozen desktop waiting in the overlay.
pub struct Pending {
    pub file: PathBuf,
    /// The main window was visible before (shown again afterwards).
    pub main_visible: bool,
    pub hud_visible: bool,
}

fn set_visible(app: &AppHandle, label: &str, visible: bool) {
    let (app2, label) = (app.clone(), label.to_string());
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = app2.get_webview_window(&label) {
            let _ = if visible { w.show() } else { w.hide() };
        }
    });
}

fn visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

/// The whole desktop as PNG: FFmpeg one frame on X11/Windows, the screenshot portal on Wayland.
fn grab_desktop(app: &AppHandle, out: &Path) -> Result<()> {
    let state = app.state::<AppState>();
    let backend = state.capture.as_ref().map_err(|why| AppError::Unsupported(why.clone()))?.clone();
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    #[cfg(target_os = "linux")]
    if backend.kind() == enrec_core::capture::BackendKind::WaylandPortal {
        // Non-interactive: the desktop may ask once whether ENRecorder may take screenshots.
        let file = enrec_core::capture::wayland::portal_screenshot(false)?;
        let bytes = std::fs::read(&file)?;
        let _ = std::fs::remove_file(&file);
        if screenshot::png_size(&bytes).is_some() {
            std::fs::write(out, &bytes)?;
        } else {
            let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
            let tmp = out.with_extension("portal");
            std::fs::write(&tmp, &bytes)?;
            let input = enrec_core::capture::CaptureInput { args: vec!["-i".into(), tmp.to_string_lossy().into_owned()], width: 0, height: 0, frames: None, origin: None };
            let r = screenshot::grab(Path::new(&ffmpeg.path), &input, out);
            let _ = std::fs::remove_file(&tmp);
            r?;
        }
        return Ok(());
    }
    let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
    let desktop = backend.list_sources()?.into_iter().find(|s| matches!(s, CaptureSource::Desktop { .. })).ok_or_else(|| AppError::NotFound("no screen to capture".into()))?;
    let input = backend.input(&CaptureRequest { source: desktop, fps: 5, show_cursor: false })?;
    screenshot::grab(Path::new(&ffmpeg.path), &input, out)?;
    Ok(())
}

/// The overlay covers every monitor (X11/Windows: the bounding box of all of them, which is what
/// the desktop capture contains) or the current one in full screen (Wayland).
fn open_overlay(app: &AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let monitors = app.available_monitors().map_err(tauri_err)?;
    let wayland = app.state::<AppState>().capture.as_ref().is_ok_and(|b| b.kind() == enrec_core::capture::BackendKind::WaylandPortal);
    let b = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=quick".into()))
        .title("ENRecorder quick capture")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(true);
    let w = b.build().map_err(tauri_err)?;
    // Place it explicitly (also works without a window manager), then ask for full screen where
    // one monitor is covered (hides panels on GNOME/KDE/Windows).
    let (x0, y0, x1, y1) = if wayland || monitors.len() <= 1 {
        let m = w.current_monitor().ok().flatten().or_else(|| monitors.first().cloned());
        m.map_or((0, 0, 1920, 1080), |m| (m.position().x, m.position().y, m.position().x + m.size().width as i32, m.position().y + m.size().height as i32))
    } else {
        (
            monitors.iter().map(|m| m.position().x).min().unwrap_or(0),
            monitors.iter().map(|m| m.position().y).min().unwrap_or(0),
            monitors.iter().map(|m| m.position().x + m.size().width as i32).max().unwrap_or(1920),
            monitors.iter().map(|m| m.position().y + m.size().height as i32).max().unwrap_or(1080),
        )
    };
    let _ = w.set_position(PhysicalPosition::new(x0, y0));
    let _ = w.set_size(PhysicalSize::new((x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32));
    if wayland || monitors.len() <= 1 {
        let _ = w.set_fullscreen(true);
    }
    // Shown by the page once the image is on screen (no flash of an empty window).
    let _ = w.set_always_on_top(true);
    Ok(())
}

/// Shortcut, tray and the Screenshot Studio button.
#[tauri::command]
pub async fn start_quick_capture(app: AppHandle) -> Result<()> {
    {
        let state = app.state::<AppState>();
        if state.quick.lock().is_some() {
            if let Some(w) = app.get_webview_window(LABEL) {
                let _ = w.set_focus();
            }
            return Ok(());
        }
    }
    let app2 = app.clone();
    blocking(move || {
        let state = app2.state::<AppState>();
        let main_visible = visible(&app2, "main");
        let hud_visible = visible(&app2, "hud");
        set_visible(&app2, "main", false);
        set_visible(&app2, "hud", false);
        // Let the windows disappear (compositor animation).
        std::thread::sleep(Duration::from_millis(if main_visible || hud_visible { 350 } else { 60 }));
        let file = state.data_dir.join("tmp").join(format!("quick-{}.png", enrec_core::database::new_id()));
        if let Err(e) = grab_desktop(&app2, &file) {
            restore(&app2, main_visible, hud_visible);
            return Err(e);
        }
        *state.quick.lock() = Some(Pending { file, main_visible, hud_visible });
        Ok(())
    })
    .await?;
    // Windows can be created from an async command (not from inside a main-thread callback).
    if let Err(e) = open_overlay(&app) {
        log::warn!("quick capture overlay: {e}");
        finish(&app);
        return Err(e);
    }
    log::info!("quick capture overlay open");
    Ok(())
}

fn restore(app: &AppHandle, main: bool, hud: bool) {
    if hud {
        set_visible(app, "hud", true);
    }
    if main {
        crate::tray::show_main(app);
    }
}

/// Closes the overlay, removes the frozen image and brings back what was visible.
fn finish(app: &AppHandle) {
    let pending = app.state::<AppState>().quick.lock().take();
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
    if let Some(p) = pending {
        let _ = std::fs::remove_file(&p.file);
        restore(app, p.main_visible, p.hud_visible);
    }
}

/// The frozen desktop (PNG bytes) for the overlay.
#[tauri::command]
pub async fn quick_capture_image(app: AppHandle) -> Result<tauri::ipc::Response> {
    blocking(move || {
        let file = app.state::<AppState>().quick.lock().as_ref().map(|p| p.file.clone()).ok_or_else(|| AppError::NotFound("no quick capture is waiting".into()))?;
        Ok(tauri::ipc::Response::new(std::fs::read(file)?))
    })
    .await
}

#[tauri::command]
pub fn quick_capture_cancel(app: AppHandle) {
    log::info!("quick capture cancelled");
    finish(&app);
}

/// `action`: "copy" (clipboard), "save" (library, also copied when `copy` is set) or "edit"
/// (library, then the Screenshot Studio editor). `png` is the finished area (base64), `annotations`
/// the editor document (items + crop) on the full capture. Returns the screenshot id when saved.
#[tauri::command]
pub async fn quick_capture_finish(app: AppHandle, action: String, png: String, annotations: String, copy: bool) -> Result<Option<String>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(png.split_once(',').map_or(png.as_str(), |(_, d)| d).trim()).map_err(|e| AppError::InvalidSetting(format!("the image data is not valid: {e}")))?;
    let (w, h) = screenshot::png_size(&bytes).ok_or_else(|| AppError::InvalidSetting("the image is not a PNG".into()))?;
    if action == "copy" || copy {
        let image = tauri::image::Image::from_bytes(&bytes).map_err(tauri_err)?;
        app.clipboard().write_image(&image).map_err(tauri_err)?;
    }
    if action == "copy" {
        log::info!("quick capture copied ({w}×{h})");
        finish(&app);
        return Ok(None);
    }
    if annotations.len() > 5_000_000 {
        return Err(AppError::InvalidSetting("the annotations are too large".into()));
    }
    let app2 = app.clone();
    let id = blocking(move || {
        let state = app2.state::<AppState>();
        let original = state.quick.lock().as_ref().map(|p| p.file.clone()).ok_or_else(|| AppError::NotFound("no quick capture is waiting".into()))?;
        let folder = state.screenshots_folder();
        enrec_core::settings::check_output_folder(&folder.to_string_lossy())?;
        let (format, quality) = {
            let s = state.settings.read();
            (s.shot_format.clone(), s.shot_quality)
        };
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let name = super::screenshot::stem(&app2, Some(&CaptureSource::Region { x: 0, y: 0, width: w, height: h }));
        let path = enrec_core::recording::unique_path(&folder, &name, screenshot::ext(&format));
        let size = screenshot::export(Path::new(&ffmpeg.path), &bytes, &format, quality, &path)?;
        let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(name);
        let id = state.db.insert_screenshot(&title, &path.to_string_lossy(), &format, w, h, size)?;
        // The full capture becomes the original, so the Studio can change the area later.
        let named = screenshot::originals_dir(&folder).join(format!("{id}.png"));
        std::fs::create_dir_all(named.parent().unwrap_or(Path::new(".")))?;
        if std::fs::rename(&original, &named).is_err() {
            std::fs::copy(&original, &named)?;
        }
        state.db.update_screenshot(&id, &path.to_string_lossy(), &format, w, h, size, Some(&annotations))?;
        log::info!("quick capture saved: {}", path.display());
        Ok(id)
    })
    .await?;
    let main_was_visible = app.state::<AppState>().quick.lock().as_ref().is_some_and(|p| p.main_visible);
    finish(&app);
    if action == "edit" {
        crate::tray::show_main(&app);
        let _ = app.emit_to("main", EV_OPEN_SHOT, &id);
    } else if !main_was_visible {
        super::recording::notify(&app, "Screenshot saved", &format!("{w}×{h}"));
    }
    Ok(Some(id))
}
