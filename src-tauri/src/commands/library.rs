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
        if u.status == "completed" {
            let name = Path::new(&u.output_path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            super::recording::notify(&self.0, "Export finished", &name);
        }
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
        let _ = std::fs::remove_file(state.tracks_dir().join(format!("{id}.json")));
        state.db.delete_media(&id)
    })
    .await
}

/// Bulk delete: every item to the trash; returns how many were removed. Stops at the first
/// error so nothing is half-done silently.
#[tauri::command]
pub async fn delete_media_many(app: AppHandle, ids: Vec<String>) -> Result<usize> {
    let mut n = 0;
    for id in ids {
        delete_media(app.clone(), id).await?;
        n += 1;
    }
    Ok(n)
}

#[tauri::command]
pub async fn list_tags(app: AppHandle) -> Result<Vec<enrec_core::database::Tag>> {
    blocking(move || app.state::<AppState>().db.list_tags()).await
}

#[tauri::command]
pub async fn create_tag(app: AppHandle, name: String, color: Option<String>) -> Result<enrec_core::database::Tag> {
    blocking(move || app.state::<AppState>().db.create_tag(&name, color.as_deref())).await
}

#[tauri::command]
pub async fn delete_tag(app: AppHandle, id: String) -> Result<()> {
    blocking(move || app.state::<AppState>().db.delete_tag(&id)).await
}

/// Replaces the tags of one or more items.
#[tauri::command]
pub async fn set_media_tags(app: AppHandle, ids: Vec<String>, tag_ids: Vec<String>) -> Result<()> {
    blocking(move || {
        let db = &app.state::<AppState>().db;
        for id in ids {
            let item = db.media_item(&id)?;
            db.set_media_tags(&id, &item.kind, &tag_ids)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn media_tag_ids(app: AppHandle, id: String) -> Result<Vec<String>> {
    blocking(move || app.state::<AppState>().db.media_tag_ids(&id)).await
}

/// Marks items whose file was moved or deleted outside ENRecorder (and restores returned ones).
#[tauri::command]
pub async fn check_library_files(app: AppHandle) -> Result<(usize, usize)> {
    blocking(move || app.state::<AppState>().db.refresh_missing()).await
}

/// Reads the file again (size, duration, codecs) and makes a new thumbnail.
#[tauri::command]
pub async fn reprocess_media(app: AppHandle, id: String) -> Result<MediaItem> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        let path = PathBuf::from(&item.path);
        if !path.is_file() {
            state.db.refresh_missing()?;
            return Err(AppError::NotFound(format!("{} is missing", path.display())));
        }
        let _ = std::fs::remove_file(enrec_core::media::thumbs::path_for(&state.thumbs_dir(), &id));
        if item.kind != "screenshot" {
            let ffprobe = state.tools.read().ffprobe.clone().ok_or(AppError::FfmpegMissing)?;
            let pr = enrec_core::media::probe::probe(Path::new(&ffprobe.path), &path)?;
            let size = std::fs::metadata(&path)?.len() as i64;
            state.db.update_probe(&id, size, &pr)?;
        }
        state.db.media_item(&id)
    })
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    clicks: bool,
    keys: bool,
}

/// Whether a recording has a pointer track (for click highlights / keystrokes on export).
#[tauri::command]
pub fn media_track(state: tauri::State<AppState>, id: String) -> Option<TrackInfo> {
    let t = enrec_core::track::Track::load(&state.tracks_dir().join(format!("{id}.json"))).ok()?;
    Some(TrackInfo { clicks: !t.events.is_empty(), keys: t.has_keys() })
}

/// Saves the frame at `at_ms` as a PNG screenshot (library + Screenshot Studio).
#[tauri::command]
pub async fn extract_frame(app: AppHandle, id: String, at_ms: u64) -> Result<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        if item.kind != "video" {
            return Err(AppError::InvalidSetting("only videos have frames".into()));
        }
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let dir = state.screenshots_folder();
        std::fs::create_dir_all(&dir)?;
        let secs = at_ms / 1000;
        let stem = format!("{} frame {}m{:02}s", item.title, secs / 60, secs % 60);
        let dst = enrec_core::recording::unique_path(&dir, &stem, "png");
        enrec_core::export::extract_frame(Path::new(&ffmpeg.path), Path::new(&item.path), at_ms, &dst)?;
        let size = std::fs::metadata(&dst)?.len();
        let (w, h) = std::fs::read(&dst).ok().and_then(|b| enrec_core::screenshot::png_size(&b)).unwrap_or((item.width.unwrap_or(0) as u32, item.height.unwrap_or(0) as u32));
        let title = dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(stem);
        state.db.insert_screenshot(&title, &dst.to_string_lossy(), "png", w, h, size)
    })
    .await
}

/// About how big an export will be.
#[tauri::command]
pub async fn estimate_export(app: AppHandle, id: String, preset: ExportPreset) -> Result<Option<u64>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        let meta = state.db.media_metadata(&id)?;
        let dur = item.duration_ms.map(|d| d as u64);
        let out = match (preset.start_ms, preset.end_ms) {
            (st, Some(en)) => Some(en.saturating_sub(st.unwrap_or(0))),
            (Some(st), None) => dur.map(|d| d.saturating_sub(st)),
            _ => dur,
        }
        .map(|m| (m as f64 / if preset.speed > 0.0 { preset.speed } else { 1.0 }) as u64);
        let fps = meta.as_ref().and_then(|m| m.fps).unwrap_or(30.0);
        let has_audio = meta.as_ref().is_none_or(|m| m.audio_streams > 0);
        Ok(enrec_core::export::estimate_bytes(&preset, out, item.width.unwrap_or(1280) as u32, item.height.unwrap_or(720) as u32, fps, has_audio))
    })
    .await
}

#[tauri::command]
pub async fn retry_export(app: AppHandle, id: String) -> Result<String> {
    blocking(move || app.state::<AppState>().exports.retry(&id)).await
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
        if !state.settings.read().thumbnails {
            return Err(AppError::Other("thumbnails are turned off (Settings → Performance)".into()));
        }
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
        let mut preset = preset;
        preset.overlay_path = None; // only ENRecorder makes overlays
        if has_video && (preset.click_highlights || preset.keystrokes) {
            let track = enrec_core::track::Track::load(&state.tracks_dir().join(format!("{id}.json"))).map_err(|_| AppError::NotFound("this recording has no pointer track (turn on \"Remember clicks\" before recording)".into()))?;
            enrec_core::export::attach_overlay(&mut preset, &track)?;
        }
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

/// Silent parts of a recording's sound, as [start, end] ms (end = null: to the end).
#[tauri::command]
pub async fn detect_silence(app: AppHandle, id: String) -> Result<Vec<(u64, Option<u64>)>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let item = state.db.media_item(&id)?;
        let ffmpeg = state.tools.read().ffmpeg.clone().ok_or(AppError::FfmpegMissing)?;
        let r = enrec_core::export::detect_silence(Path::new(&ffmpeg.path), Path::new(&item.path), -45.0, 700)?;
        Ok(r.into_iter().map(|(a, b)| (a, (b != u64::MAX).then_some(b))).collect())
    })
    .await
}
