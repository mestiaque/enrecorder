//! Recordings library and export queue (read side; editing and exporting arrive in phase 5).

use super::blocking;
use crate::state::AppState;
use enrec_core::database::{ExportJobRow, LibraryQuery, MediaItem};
use enrec_core::Result;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub async fn list_media(app: AppHandle, query: LibraryQuery) -> Result<Vec<MediaItem>> {
    blocking(move || app.state::<AppState>().db.list_media(&query)).await
}

#[tauri::command]
pub async fn list_export_jobs(app: AppHandle) -> Result<Vec<ExportJobRow>> {
    blocking(move || app.state::<AppState>().db.list_export_jobs(200)).await
}
