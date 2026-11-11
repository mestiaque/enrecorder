//! Structured errors. Each variant has a short, plain message for the UI (`Display`) and a
//! stable `kind` the UI can switch on (icon, suggested action). Technical detail goes to the log.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error("FFmpeg was not found. Install it (Ubuntu: sudo apt install ffmpeg) or choose it in Settings → Tools")]
    FfmpegMissing,
    #[error("FFmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("Not enough disk space")]
    DiskFull,
    #[error("Invalid folder: {0}")]
    InvalidPath(String),
    #[error("Invalid setting: {0}")]
    InvalidSetting(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Not supported on this system: {0}")]
    Unsupported(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Db(String),
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::FfmpegMissing => "ffmpegMissing",
            AppError::Ffmpeg(_) => "ffmpeg",
            AppError::DiskFull => "diskFull",
            AppError::InvalidPath(_) => "invalidPath",
            AppError::InvalidSetting(_) => "invalidSetting",
            AppError::PermissionDenied(_) => "permissionDenied",
            AppError::Unsupported(_) => "unsupported",
            AppError::NotFound(_) => "notFound",
            AppError::Db(_) => "db",
            AppError::Io(_) => "io",
            AppError::Other(_) => "other",
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        // ENOSPC (28) on Unix; ERROR_DISK_FULL (112) / ERROR_HANDLE_DISK_FULL (39) on Windows.
        match (e.kind(), e.raw_os_error()) {
            (_, Some(28)) if cfg!(unix) => AppError::DiskFull,
            (_, Some(112)) | (_, Some(39)) if cfg!(windows) => AppError::DiskFull,
            (std::io::ErrorKind::PermissionDenied, _) => AppError::PermissionDenied(e.to_string()),
            _ => AppError::Io(e.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(format!("Invalid data: {e}"))
    }
}

/// Reaches the frontend as `{ kind, message }`.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let v = serde_json::to_value(AppError::FfmpegMissing).unwrap();
        assert_eq!(v["kind"], "ffmpegMissing");
        assert!(v["message"].as_str().unwrap().contains("FFmpeg"));
    }

    #[test]
    fn io_errors_are_classified() {
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(AppError::from(denied).kind(), "permissionDenied");
        #[cfg(unix)]
        assert_eq!(AppError::from(std::io::Error::from_raw_os_error(28)), AppError::DiskFull);
    }
}
