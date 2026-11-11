//! Tauri commands, one module per area. Each returns `Result<_, AppError>`, which reaches the
//! UI as `{ kind, message }`. Commands are thin: the work happens in `enrec-core`.

pub mod dashboard;
pub mod library;
pub mod media;
pub mod settings;
pub mod system;

pub(crate) fn tauri_err(e: impl std::fmt::Display) -> enrec_core::AppError {
    enrec_core::AppError::Other(e.to_string())
}

/// Runs blocking work (process spawns, folder walks, SQLite) off the async runtime threads.
pub(crate) async fn blocking<T: Send + 'static>(f: impl FnOnce() -> enrec_core::Result<T> + Send + 'static) -> enrec_core::Result<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(tauri_err)?
}
