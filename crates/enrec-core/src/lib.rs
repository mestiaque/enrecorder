//! ENRecorder core: everything that does not need a window. The SQLite store, typed settings,
//! the FFmpeg service, system detection, storage accounting and the capture interfaces. Unit
//! tested without Tauri; platform code sits in `cfg`-gated files.

pub mod audio;
pub mod capture;
pub mod database;
pub mod error;
pub mod media;
pub mod recording;
pub mod settings;
pub mod storage;
pub mod system;

pub use error::{AppError, Result};
