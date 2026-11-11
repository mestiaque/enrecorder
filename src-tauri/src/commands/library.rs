//! Recordings library (items, rename, delete to the trash, favourites, thumbnails) and the
//! export queue (start, cancel, progress events).

use super::blocking;
use crate::state::AppState;
use enrec_core::database::{ExportJobRow, LibraryQuery, MediaItem};
use enrec_core::export::{ExportObserver, ExportPreset, ExportUpdate};
use enrec_core::media::probe::Probe;
use enrec_core::{AppError, Result};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

pub const EV_EXPORT: &str = "export-progress";

pub struct ExportEvents(pub AppHandle);
impl ExportObserver for ExportEvents {
    fn update(&self, u: &ExportUpdate) {
        let _ = self.0.emit(EV_EXPORT, u);
    }
}

#[tauri::command]
pub async fn list_media(app: AppHandle, query: LibraryQuery) -> Result<Vec<MediaItem>> {
    blocking(move || app.state::<AppState>().db.list_media(&query)).await
}

#[tauri::command]
pub async fn list_export_jobs(app: AppHandle) -> Result<Vec<ExportJobRow>> {
    blocking(move || app.state::<AppState>().db.list_export_jobs(200)).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetail {
    item: MediaItem,
    metadata: Option<Probe>,
    exists: bool,
}

#[tauri::command]
pub async fn get_media(app: AppHandle, id: String) -> Result<MediaDetail> {
    blocking(move || {
        let db = &app.state::<AppState>().db;
        let item = db.media_item(&id)?;
        let exists = Path::new(&item.path).is_file();
        Ok(MediaDetail { metadata: db.media_metadata(&id)?, exists, item })
    })
    .await
}

/// Renames the file on disk (same folder, same extension) and the library entry.
#[tauri::command]
pub async fn rename_media(app: AppHandle, id: String, title: String) -> Result<MediaItem> {
    blocking(move || {
        let db = &app.state::<AppState>().db;
        let item = db.media_item(&id)?;
        let title = title.trim().to_string();
        if title.is_empty() || title.len() > 200 || title.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) || title.starts_with('.') {
            return Err(AppError::InvalidSetting("Use a name without / \\ : * ? \" < > |".into()));
        }
        let old = PathBuf::from(&item.path);
        let ext = old.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let new = old.with_file_name(if ext.is_empty() { title.clone() } else { format!("{title}.{ext}") });
        if new != old {
            if new.exists() {
                return Err(AppError::InvalidSetting(format!("{} already exists", new.display())));
            }
            std::fs::rename(&old, &new)?;
        }
        db.rename_media(&id, &title, &new.to_string_lossy())?;
        db.media_item(&id)
    })
    .await
}

/// Moves the file to the system trash (recoverable) and removes the library entry.
#[tauri::command]
pub async fn delete_media(app: AppHandle, id: String) -> Result<()> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        let p = PathBuf::from(&item.path);
        if p.exists() {
            trash::delete(&p).map_err(|e| AppError::Io(format!("could not move {} to the trash: {e}", p.display())))?;
        }
        if item.kind == "screenshot" {
            let _ = std::fs::remove_file(enrec_core::screenshot::originals_dir(&state.screenshots_folder()).join(format!("{id}.png")));
        }
        let _ = std::fs::remove_file(enrec_core::media::thumbs::path_for(&state.thumbs_dir(), &id));
        state.db.delete_media(&id)
    })
    .await
}

#[tauri::command]
pub async fn set_favorite(app: AppHandle, id: String, on: bool) -> Result<()> {
    blocking(move || {
        let db = &app.state::<AppState>().db;
        let item = db.media_item(&id)?;
        db.set_favorite(&id, &item.kind, on)
    })
    .await
}

/// Path of the item's thumbnail (made on first use).
#[tauri::command]
pub async fn media_thumbnail(app: AppHandle, id: String) -> Result<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let t = enrec_core::media::thumbs::thumbnail(Path::new(&ffmpeg.path), &state.thumbs_dir(), &id, &item.kind, Path::new(&item.path), item.duration_ms)?;
        Ok(t.to_string_lossy().into_owned())
    })
    .await
}

/// Queues an export of a library item; returns the job id.
#[tauri::command]
pub async fn start_export(app: AppHandle, id: String, preset: ExportPreset) -> Result<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        if item.kind == "screenshot" {
            return Err(AppError::InvalidSetting("screenshots are exported from the Screenshot editor".into()));
        }
        let suffix = match preset.kind.as_str() {
            "trim" => " (edited)",
            "compress" => " (compressed)",
            "audio" => " (audio)",
            _ => "",
        };
        let has_video = item.kind == "video";
        state.exports.enqueue(Some(&id), Path::new(&item.path), &state.exports_folder(), &format!("{}{suffix}", item.title), preset, item.duration_ms.map(|d| d as u64), has_video)
    })
    .await
}

#[tauri::command]
pub fn cancel_export(state: tauri::State<AppState>, id: String) -> Result<()> {
    state.exports.cancel(&id)
}

/// Address the preview player loads a recording from (see media_server).
#[tauri::command]
pub fn media_url(id: String) -> Result<String> {
    crate::media_server::url(&id).ok_or_else(|| AppError::Other("the preview server is not running".into()))
}
