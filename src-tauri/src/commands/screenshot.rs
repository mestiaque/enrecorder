//! Screenshot commands: capture (FFmpeg one frame, or the Wayland screenshot portal), the image
//! for the editor, saving the edited result, and copying it to the clipboard.

use super::{blocking, tauri_err};
use crate::state::AppState;
use base64::Engine;
use enrec_core::capture::{CaptureRequest, CaptureSource};
use enrec_core::screenshot::{self, Screenshot};
use enrec_core::{AppError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// The file name from Settings → Screenshots (template with {date}, {time}, {mode}).
fn stem(app: &AppHandle, source: Option<&CaptureSource>) -> String {
    let now = tauri_plugin_log::TimezoneStrategy::UseLocal.get_now();
    let date = now.format(time::macros::format_description!("[year]-[month]-[day]")).unwrap_or_default();
    let time = now.format(time::macros::format_description!("[hour]-[minute]-[second]")).unwrap_or_default();
    let mode = source.map_or("screen", |s| match s.kind_label() {
        "desktop" | "monitor" => "screen",
        other => other,
    });
    let template = app.state::<AppState>().settings.read().shot_name_template.clone();
    enrec_core::settings::expand_template(&template, &date, &time, mode)
}

fn set_main_visible(app: &AppHandle, visible: bool) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = app2.get_webview_window("main") {
            let _ = if visible { w.show().and_then(|_| w.set_focus()) } else { w.hide() };
        }
    });
}

/// `source`: what to capture (desktop, monitor or window; an area is cropped in the editor
/// from a desktop capture). `None` on Wayland = the desktop's own screenshot dialog.
#[tauri::command]
pub async fn take_screenshot(app: AppHandle, source: Option<CaptureSource>, delay: u32, hide_window: bool, cursor: bool) -> Result<Screenshot> {
    blocking(move || {
        let state = app.state::<AppState>();
        let folder = state.screenshots_folder();
        enrec_core::settings::check_output_folder(&folder.to_string_lossy())?;
        let name = stem(&app, source.as_ref());
        let original = screenshot::originals_dir(&folder).join(format!("{}.png", enrec_core::database::new_id()));
        if hide_window {
            set_main_visible(&app, false);
        }
        // Let the window disappear (compositor animation) and honour the delay.
        std::thread::sleep(Duration::from_millis(350) + Duration::from_secs(delay.min(10) as u64));
        let result = capture(&app, source, cursor, &original);
        if hide_window {
            set_main_visible(&app, true);
        }
        let (w, h) = result?;
        let png = std::fs::read(&original)?;
        let path = enrec_core::recording::unique_path(&folder, &name, "png");
        std::fs::write(&path, &png)?;
        let id = state.db.insert_screenshot(&name, &path.to_string_lossy(), "png", w, h, png.len() as u64)?;
        // The original is named after the row so the editor can find it.
        let named = screenshot::originals_dir(&folder).join(format!("{id}.png"));
        std::fs::rename(&original, &named)?;
        state.db.screenshot(&id)
    })
    .await
}

fn capture(app: &AppHandle, source: Option<CaptureSource>, cursor: bool, out: &Path) -> Result<(u32, u32)> {
    let state = app.state::<AppState>();
    let backend = state.capture.as_ref().map_err(|why| AppError::Unsupported(why.clone()))?.clone();
    match source {
        #[cfg(target_os = "linux")]
        None | Some(CaptureSource::Portal) => {
            let file = enrec_core::capture::wayland::portal_screenshot(true)?;
            let bytes = std::fs::read(&file)?;
            // Portals may answer with PNG (GNOME, KDE) — anything else is converted by FFmpeg.
            let bytes = match screenshot::png_size(&bytes) {
                Some(_) => bytes,
                None => {
                    let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
                    let input = enrec_core::capture::CaptureInput { args: vec!["-i".into(), file.to_string_lossy().into_owned()], width: 0, height: 0, frames: None, origin: None };
                    screenshot::grab(Path::new(&ffmpeg.path), &input, out)?;
                    std::fs::read(out)?
                }
            };
            let _ = std::fs::remove_file(&file);
            std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
            std::fs::write(out, &bytes)?;
            screenshot::png_size(&bytes).ok_or_else(|| AppError::Other("the screenshot is not a readable image".into()))
        }
        #[cfg(not(target_os = "linux"))]
        None | Some(CaptureSource::Portal) => Err(AppError::Unsupported("choose a screen or window".into())),
        Some(src) => {
            let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
            let input = backend.input(&CaptureRequest { source: src, fps: 5, show_cursor: cursor })?;
            screenshot::grab(Path::new(&ffmpeg.path), &input, out)
        }
    }
}

/// The untouched capture (base64 PNG) the editor draws on; falls back to the visible file.
#[tauri::command]
pub async fn screenshot_image(app: AppHandle, id: String) -> Result<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let shot = state.db.screenshot(&id)?;
        let original = screenshot::originals_dir(&state.screenshots_folder()).join(format!("{id}.png"));
        let file = if original.is_file() { original } else { PathBuf::from(&shot.path) };
        Ok(B64.encode(std::fs::read(&file).map_err(|e| AppError::NotFound(format!("{}: {e}", file.display())))?))
    })
    .await
}

fn decode(png_base64: &str) -> Result<Vec<u8>> {
    let data = png_base64.split_once(',').map_or(png_base64, |(_, d)| d);
    B64.decode(data.trim()).map_err(|e| AppError::InvalidSetting(format!("the image data is not valid: {e}")))
}

/// Saves the editor's flattened image (PNG from the canvas) as PNG, JPEG or WebP and stores the
/// annotation document so the screenshot can be edited again.
#[tauri::command]
pub async fn save_screenshot(app: AppHandle, id: String, png: String, format: String, quality: u32, annotations: Option<String>) -> Result<Screenshot> {
    blocking(move || {
        let state = app.state::<AppState>();
        let shot = state.db.screenshot(&id)?;
        let bytes = decode(&png)?;
        let (w, h) = screenshot::png_size(&bytes).ok_or_else(|| AppError::InvalidSetting("the image to save is not a PNG".into()))?;
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let old = PathBuf::from(&shot.path);
        let target = old.with_extension(screenshot::ext(&format));
        let size = screenshot::export(Path::new(&ffmpeg.path), &bytes, &format, quality, &target)?;
        if target != old {
            let _ = std::fs::remove_file(&old);
        }
        if annotations.as_ref().is_some_and(|a| a.len() > 5_000_000) {
            return Err(AppError::InvalidSetting("the annotations are too large".into()));
        }
        state.db.update_screenshot(&id, &target.to_string_lossy(), &format, w, h, size, annotations.as_deref())?;
        state.db.screenshot(&id)
    })
    .await
}

/// Copies a PNG (the editor's current image) to the clipboard.
#[tauri::command]
pub fn copy_screenshot(app: AppHandle, png: String) -> Result<()> {
    let bytes = decode(&png)?;
    let image = tauri::image::Image::from_bytes(&bytes).map_err(tauri_err)?;
    app.clipboard().write_image(&image).map_err(tauri_err)
}

#[tauri::command]
pub async fn get_screenshot(app: AppHandle, id: String) -> Result<Screenshot> {
    blocking(move || app.state::<AppState>().db.screenshot(&id)).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrStatus {
    available: bool,
    languages: Vec<String>,
}

/// Is Tesseract installed, and with which languages.
#[tauri::command]
pub async fn ocr_status() -> Result<OcrStatus> {
    blocking(|| {
        Ok(match screenshot::ocr::find_tesseract() {
            Some(t) => OcrStatus { available: true, languages: screenshot::ocr::languages(&t) },
            None => OcrStatus { available: false, languages: vec![] },
        })
    })
    .await
}

/// Text in the editor's current image (PNG) — all on this computer.
#[tauri::command]
pub async fn ocr_screenshot(app: AppHandle, png: String, languages: String) -> Result<String> {
    blocking(move || {
        let tess = screenshot::ocr::find_tesseract().ok_or_else(|| AppError::Unsupported(if cfg!(windows) { "Install Tesseract OCR (UB Mannheim build) to read text from images".into() } else { "Install Tesseract to read text from images: sudo apt install tesseract-ocr".into() }))?;
        let bytes = decode(&png)?;
        let tmp = app.state::<AppState>().data_dir.join(format!("ocr-{}.png", enrec_core::database::new_id()));
        std::fs::write(&tmp, &bytes)?;
        let r = screenshot::ocr::recognize(&tess, &tmp, &languages);
        let _ = std::fs::remove_file(&tmp);
        r
    })
    .await
}

/// Copies text (OCR result) to the clipboard.
#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<()> {
    app.clipboard().write_text(text).map_err(tauri_err)
}
