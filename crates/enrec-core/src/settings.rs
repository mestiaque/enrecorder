//! Typed settings. Stored in SQLite as one JSON value per key (like the other EN* apps), so a
//! new setting needs no migration, and an invalid stored value only resets that one key.
//!
//! Only settings that something in the app actually uses live here; recording, audio and
//! screenshot defaults are added with the phases that use them.

use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    // General
    pub start_with_os: bool,
    pub start_minimized: bool,
    /// Closing the window hides it to the tray (recordings keep running there).
    pub close_to_tray: bool,
    /// "en" (more languages later).
    pub language: String,
    // Storage — empty means the default folder (see [`Settings::recordings_folder`]).
    pub recordings_dir: String,
    pub screenshots_dir: String,
    // Tools — empty means "find automatically".
    pub ffmpeg_path: String,
    // Appearance
    /// "dark" | "light" | "glass" | "system"
    pub theme: String,
    pub accent: String,
    pub animations: bool,
    pub sidebar_collapsed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_with_os: false,
            start_minimized: false,
            close_to_tray: true,
            language: "en".into(),
            recordings_dir: String::new(),
            screenshots_dir: String::new(),
            ffmpeg_path: String::new(),
            theme: "dark".into(),
            accent: "#6366f1".into(),
            animations: true,
            sidebar_collapsed: false,
        }
    }
}

/// Keys whose value is a folder; checked before they are stored.
pub const FOLDER_KEYS: &[&str] = &["recordingsDir", "screenshotsDir"];

impl Settings {
    /// Build from stored key → JSON pairs, key by key.
    pub fn from_map(stored: &HashMap<String, Value>) -> Self {
        let defaults = Settings::default();
        let Ok(Value::Object(mut base)) = serde_json::to_value(&defaults) else { return defaults };
        for (k, v) in stored {
            if !base.contains_key(k) {
                continue;
            }
            let mut trial: Map<String, Value> = base.clone();
            trial.insert(k.clone(), v.clone());
            if serde_json::from_value::<Settings>(Value::Object(trial.clone())).is_ok() {
                base = trial;
            } else {
                log::warn!("ignoring invalid stored setting {k}");
            }
        }
        serde_json::from_value::<Settings>(Value::Object(base)).unwrap_or(defaults).validated()
    }

    /// Fix enums and formats so the rest of the app can trust the values.
    pub fn validated(mut self) -> Self {
        let d = Settings::default();
        if !matches!(self.theme.as_str(), "dark" | "light" | "glass" | "system") {
            self.theme = d.theme;
        }
        if !is_hex_color(&self.accent) {
            self.accent = d.accent;
        }
        if !matches!(self.language.as_str(), "en") {
            self.language = d.language;
        }
        for dir in [&mut self.recordings_dir, &mut self.screenshots_dir, &mut self.ffmpeg_path] {
            *dir = dir.trim().to_string();
            if !dir.is_empty() && !Path::new(dir.as_str()).is_absolute() {
                dir.clear();
            }
        }
        self
    }

    pub fn is_known_key(key: &str) -> bool {
        matches!(serde_json::to_value(Settings::default()), Ok(Value::Object(m)) if m.contains_key(key))
    }

    /// Where recordings are saved: the folder from Settings, or `<Videos>/ENRecorder`.
    pub fn recordings_folder(&self, system_videos: &Path) -> PathBuf {
        if self.recordings_dir.is_empty() {
            system_videos.join("ENRecorder")
        } else {
            PathBuf::from(&self.recordings_dir)
        }
    }

    /// Where screenshots are saved: the folder from Settings, or `<Pictures>/ENRecorder`.
    pub fn screenshots_folder(&self, system_pictures: &Path) -> PathBuf {
        if self.screenshots_dir.is_empty() {
            system_pictures.join("ENRecorder")
        } else {
            PathBuf::from(&self.screenshots_dir)
        }
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Checks a folder chosen for saving: absolute, an existing directory (or creatable) and
/// writable. Empty means "use the default" and is always fine.
pub fn check_output_folder(value: &str) -> Result<()> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(());
    }
    let p = Path::new(v);
    if !p.is_absolute() {
        return Err(AppError::InvalidPath(format!("{v} is not a full path")));
    }
    if p.exists() && !p.is_dir() {
        return Err(AppError::InvalidPath(format!("{v} is a file, not a folder")));
    }
    std::fs::create_dir_all(p).map_err(|e| AppError::InvalidPath(format!("{v} cannot be created: {e}")))?;
    // Prove it is writable with a real file, then remove it.
    let probe = p.join(format!(".enrecorder-write-test-{}", std::process::id()));
    std::fs::write(&probe, b"ok").map_err(|e| AppError::InvalidPath(format!("{v} is not writable: {e}")))?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_roundtrip() {
        assert_eq!(Settings::from_map(&HashMap::new()), Settings::default());
    }

    #[test]
    fn invalid_values_fall_back_per_key() {
        let mut m = HashMap::new();
        m.insert("theme".to_string(), json!("neon"));
        m.insert("accent".to_string(), json!("blue"));
        m.insert("startMinimized".to_string(), json!("yes"));
        m.insert("closeToTray".to_string(), json!(false));
        m.insert("recordingsDir".to_string(), json!("relative/dir"));
        m.insert("unknown".to_string(), json!(1));
        let s = Settings::from_map(&m);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.accent, "#6366f1");
        assert!(!s.start_minimized);
        assert!(!s.close_to_tray);
        assert_eq!(s.recordings_dir, "");
    }

    #[test]
    fn default_folders() {
        let s = Settings::default();
        assert_eq!(s.recordings_folder(Path::new("/home/u/Videos")), PathBuf::from("/home/u/Videos/ENRecorder"));
        assert_eq!(s.screenshots_folder(Path::new("/home/u/Pictures")), PathBuf::from("/home/u/Pictures/ENRecorder"));
        let s = Settings { recordings_dir: "/media/rec".into(), ..Settings::default() };
        assert_eq!(s.recordings_folder(Path::new("/x")), PathBuf::from("/media/rec"));
    }

    #[test]
    fn output_folder_checks() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_output_folder("").is_ok());
        assert!(check_output_folder("not/absolute").is_err());
        let file = dir.path().join("f.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(check_output_folder(file.to_str().unwrap()).is_err());
        let new = dir.path().join("new/sub");
        assert!(check_output_folder(new.to_str().unwrap()).is_ok());
        assert!(new.is_dir());
        assert_eq!(std::fs::read_dir(&new).unwrap().count(), 0, "the write test file is removed");
    }

    #[test]
    fn known_keys() {
        assert!(Settings::is_known_key("recordingsDir"));
        assert!(!Settings::is_known_key("nope"));
        for k in FOLDER_KEYS {
            assert!(Settings::is_known_key(k));
        }
    }
}
