//! Storage accounting for the dashboard: how much the save folders use and how much space is
//! left on the drive that holds them.

use crate::error::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FolderUsage {
    pub path: String,
    pub exists: bool,
    pub bytes: u64,
    pub files: u64,
    /// The walk stopped at the file limit; `bytes` is a lower bound.
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpace {
    /// The folder the numbers are for (the nearest existing parent of the save folder).
    pub path: String,
    pub available: u64,
    pub total: u64,
}

const MAX_FILES: u64 = 200_000;

/// Total size of the files below `dir` (symlinks are not followed, so a link to `/` cannot
/// make this walk the whole disk).
pub fn folder_usage(dir: &Path) -> FolderUsage {
    let mut u = FolderUsage { path: dir.to_string_lossy().into_owned(), exists: dir.is_dir(), bytes: 0, files: 0, truncated: false };
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                stack.push(e.path());
            } else if ft.is_file() {
                u.bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
                u.files += 1;
                if u.files >= MAX_FILES {
                    u.truncated = true;
                    return u;
                }
            }
        }
    }
    u
}

/// The nearest existing ancestor (a save folder may not exist until the first recording).
pub fn existing_ancestor(p: &Path) -> Option<PathBuf> {
    p.ancestors().find(|a| a.exists()).map(Path::to_path_buf)
}

pub fn disk_space(p: &Path) -> Result<DiskSpace> {
    let at = existing_ancestor(p).unwrap_or_else(|| p.to_path_buf());
    Ok(DiskSpace { path: at.to_string_lossy().into_owned(), available: fs4::available_space(&at)?, total: fs4::total_space(&at)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_files_recursively() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp4"), vec![0u8; 1000]).unwrap();
        std::fs::create_dir_all(dir.path().join("sub/deeper")).unwrap();
        std::fs::write(dir.path().join("sub/deeper/b.png"), vec![0u8; 24]).unwrap();
        let u = folder_usage(dir.path());
        assert!(u.exists);
        assert_eq!((u.bytes, u.files, u.truncated), (1024, 2, false));
    }

    #[test]
    fn missing_folder_is_empty() {
        let u = folder_usage(Path::new("/definitely/not/here"));
        assert!(!u.exists);
        assert_eq!(u.bytes, 0);
    }

    #[test]
    fn disk_space_of_a_future_folder() {
        let dir = tempfile::tempdir().unwrap();
        let future = dir.path().join("ENRecorder/not/yet");
        assert_eq!(existing_ancestor(&future).as_deref(), Some(dir.path()));
        let d = disk_space(&future).unwrap();
        assert!(d.total > 0 && d.available <= d.total);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/", dir.path().join("root")).unwrap();
        assert_eq!(folder_usage(dir.path()).files, 0);
    }
}
