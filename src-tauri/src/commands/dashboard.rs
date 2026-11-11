//! Data for the Home page.

use super::blocking;
use crate::state::AppState;
use enrec_core::database::{LibraryQuery, LibraryStats, MediaItem};
use enrec_core::storage::{disk_space, folder_usage, DiskSpace, FolderUsage};
use enrec_core::Result;
use tauri::{AppHandle, Manager};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    stats: LibraryStats,
    recent_recordings: Vec<MediaItem>,
    recent_screenshots: Vec<MediaItem>,
    recordings: FolderUsage,
    screenshots: FolderUsage,
    /// Free space where recordings are saved.
    disk: Option<DiskSpace>,
}

#[tauri::command]
pub async fn dashboard(app: AppHandle) -> Result<Dashboard> {
    blocking(move || {
        let state = app.state::<AppState>();
        let rec_dir = state.recordings_folder();
        let shot_dir = state.screenshots_folder();
        let q = |filter: &str| LibraryQuery { filter: filter.into(), sort: "newest".into(), limit: 6, ..Default::default() };
        let mut recent_recordings = state.db.list_media(&q("video"))?;
        recent_recordings.extend(state.db.list_media(&q("audio"))?);
        recent_recordings.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recent_recordings.truncate(6);
        Ok(Dashboard {
            stats: state.db.library_stats()?,
            recent_recordings,
            recent_screenshots: state.db.list_media(&q("screenshot"))?,
            recordings: folder_usage(&rec_dir),
            screenshots: folder_usage(&shot_dir),
            disk: disk_space(&rec_dir).map_err(|e| log::warn!("disk space of {}: {e}", rec_dir.display())).ok(),
        })
    })
    .await
}
