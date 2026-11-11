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
    // Recording defaults (the Video Recorder form remembers the last choice here)
    pub rec_fps: u32,
    /// "native" | "720" | "1080" | "1440" | "2160"
    pub rec_resolution: String,
    /// "mp4" | "mkv" | "webm"
    pub rec_format: String,
    /// "h264" | "hevc" | "av1" | "vp9"
    pub rec_codec: String,
    /// "auto" or an encoder name (must be verified when recording starts).
    pub rec_encoder: String,
    /// "high" | "balanced" | "small"
    pub rec_quality: String,
    pub rec_cursor: bool,
    /// Seconds: 0, 3, 5 or 10.
    pub rec_countdown: u32,
    /// Minutes; 0 = no limit.
    pub rec_max_minutes: u32,
    /// Hide the main window while recording (it would otherwise be in the recording).
    pub rec_hide_window: bool,
    /// Show the floating recording controls.
    pub rec_show_hud: bool,
    /// Audio in screen recordings: device id, "" = off, "default" = system default (phase 3).
    pub rec_mic: String,
    pub rec_system_audio: String,
    /// Microphone and system audio on separate tracks instead of one mix.
    pub rec_separate_tracks: bool,
    /// Look for a new release on GitHub once a day (phase 7).
    pub check_updates: bool,
    /// Unix ms of the last automatic check.
    pub last_update_check: i64,
    /// Microphone clean-up in recordings (phase 6).
    pub rec_mic_denoise: bool,
    pub rec_mic_normalize: bool,
    /// Webcam overlay: device ("" = off), corner, size (% of width), shape.
    pub rec_webcam: String,
    pub rec_webcam_position: String,
    pub rec_webcam_size: u32,
    pub rec_webcam_shape: String,
    /// Webcam frame colour ("" = none) and shadow.
    pub rec_webcam_border: String,
    pub rec_webcam_shadow: bool,
    /// Audio Recorder: devices ("" = off), format and quality.
    pub audio_mic: String,
    pub audio_system: String,
    pub audio_format: String,
    pub audio_quality: String,
    /// Microphone noise gate, echo cancellation, input volumes (percent, 0–200).
    pub rec_mic_gate: bool,
    pub rec_echo_cancel: bool,
    pub rec_mic_volume: u32,
    pub rec_system_volume: u32,
    /// Audio capture format: 44100 / 48000 / 96000 Hz, 1 or 2 channels.
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    /// Start a new file every N minutes (0 = one file).
    pub rec_split_minutes: u32,
    /// Remember pointer positions and clicks while recording (X11/Windows), so click
    /// highlights can be added when exporting. Applied afterwards, never during capture.
    pub rec_track_clicks: bool,
    /// Remember key presses for a keystroke overlay added when exporting (off by default:
    /// it records what is typed; X11/Windows only).
    pub rec_track_keys: bool,
    // Screenshots
    /// "png" | "jpeg" | "webp"
    pub shot_format: String,
    /// JPEG/WebP quality 50–100.
    pub shot_quality: u32,
    /// File name template: {date}, {time}, {mode} are replaced.
    pub shot_name_template: String,
    // Performance
    /// Use GPU encoders when available ("Automatic" picks software when off).
    pub hw_accel: bool,
    /// Make thumbnails for the library.
    pub thumbnails: bool,
    // Privacy
    /// Delete log files older than this many days (0 = keep).
    pub log_retention_days: u32,
    /// Desktop notification when a recording, export or transcript is saved (or a recording fails).
    pub notify_on_save: bool,
    // Transcription (optional, whisper.cpp)
    /// whisper.cpp program (`whisper-cli`); empty = look in the usual places.
    pub whisper_path: String,
    /// Speech model (`ggml-*.bin`); empty = the smallest one found.
    pub whisper_model: String,
    /// Spoken language code ("en", "bn", …) or "auto".
    pub transcribe_language: String,
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
            rec_fps: 30,
            rec_resolution: "native".into(),
            rec_format: "mp4".into(),
            rec_codec: "h264".into(),
            rec_encoder: "auto".into(),
            rec_quality: "balanced".into(),
            rec_cursor: true,
            rec_countdown: 3,
            rec_max_minutes: 0,
            rec_hide_window: true,
            rec_show_hud: true,
            rec_mic: String::new(),
            rec_system_audio: String::new(),
            rec_separate_tracks: false,
            check_updates: true,
            last_update_check: 0,
            rec_mic_denoise: false,
            rec_mic_normalize: false,
            rec_webcam: String::new(),
            rec_webcam_position: "bottom-right".into(),
            rec_webcam_size: 22,
            rec_webcam_shape: "circle".into(),
            rec_webcam_border: String::new(),
            rec_webcam_shadow: false,
            audio_mic: "default".into(),
            audio_system: String::new(),
            audio_format: "mp3".into(),
            audio_quality: "balanced".into(),
            rec_mic_gate: false,
            rec_echo_cancel: false,
            rec_mic_volume: 100,
            rec_system_volume: 100,
            audio_sample_rate: 48000,
            audio_channels: 2,
            rec_split_minutes: 0,
            rec_track_clicks: false,
            rec_track_keys: false,
            shot_format: "png".into(),
            shot_quality: 90,
            shot_name_template: "Screenshot {date} {time}".into(),
            hw_accel: true,
            thumbnails: true,
            log_retention_days: 14,
            notify_on_save: true,
            whisper_path: String::new(),
            whisper_model: String::new(),
            transcribe_language: "auto".into(),
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
        if !matches!(self.rec_fps, 24 | 30 | 60 | 120) {
            self.rec_fps = d.rec_fps;
        }
        if !matches!(self.rec_resolution.as_str(), "native" | "720" | "1080" | "1440" | "2160") {
            self.rec_resolution = d.rec_resolution;
        }
        if !matches!(self.rec_format.as_str(), "mp4" | "mkv" | "webm") {
            self.rec_format = d.rec_format;
        }
        if !matches!(self.rec_codec.as_str(), "h264" | "hevc" | "av1" | "vp9") {
            self.rec_codec = d.rec_codec;
        }
        if self.rec_encoder.trim().is_empty() || self.rec_encoder.len() > 40 || !self.rec_encoder.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            self.rec_encoder = d.rec_encoder;
        }
        if !matches!(self.rec_quality.as_str(), "high" | "balanced" | "small") {
            self.rec_quality = d.rec_quality;
        }
        if !matches!(self.rec_countdown, 0 | 3 | 5 | 10) {
            self.rec_countdown = d.rec_countdown;
        }
        self.rec_max_minutes = self.rec_max_minutes.min(24 * 60);
        if !matches!(self.rec_webcam_position.as_str(), "bottom-right" | "bottom-left" | "top-right" | "top-left") {
            self.rec_webcam_position = d.rec_webcam_position.clone();
        }
        if !(self.rec_webcam_border.is_empty() || (self.rec_webcam_border.len() == 7 && self.rec_webcam_border.starts_with('#') && self.rec_webcam_border[1..].chars().all(|c| c.is_ascii_hexdigit()))) {
            self.rec_webcam_border.clear();
        }
        if !matches!(self.rec_webcam_shape.as_str(), "circle" | "rectangle" | "rounded") {
            self.rec_webcam_shape = d.rec_webcam_shape.clone();
        }
        self.rec_webcam_size = self.rec_webcam_size.clamp(10, 40);
        for dev in [&mut self.rec_mic, &mut self.rec_system_audio, &mut self.audio_mic, &mut self.audio_system, &mut self.rec_webcam] {
            if dev.len() > 300 || dev.chars().any(|c| c.is_control()) {
                dev.clear();
            }
        }
        if !crate::audio::AUDIO_FORMATS.contains(&self.audio_format.as_str()) {
            self.audio_format = d.audio_format;
        }
        if !matches!(self.audio_quality.as_str(), "high" | "balanced" | "small") {
            self.audio_quality = d.audio_quality;
        }
        self.rec_mic_volume = self.rec_mic_volume.min(200);
        self.rec_system_volume = self.rec_system_volume.min(200);
        if !crate::audio::SAMPLE_RATES.contains(&self.audio_sample_rate) {
            self.audio_sample_rate = d.audio_sample_rate;
        }
        if !matches!(self.audio_channels, 1 | 2) {
            self.audio_channels = d.audio_channels;
        }
        if !matches!(self.rec_split_minutes, 0 | 5 | 10 | 15 | 30 | 60) {
            self.rec_split_minutes = d.rec_split_minutes;
        }
        if !matches!(self.shot_format.as_str(), "png" | "jpeg" | "webp") {
            self.shot_format = d.shot_format.clone();
        }
        self.shot_quality = self.shot_quality.clamp(50, 100);
        let t = self.shot_name_template.trim();
        if t.is_empty() || t.len() > 120 || t.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
            self.shot_name_template = d.shot_name_template.clone();
        }
        self.log_retention_days = self.log_retention_days.min(365);
        if !matches!(self.language.as_str(), "en") {
            self.language = d.language;
        }
        let lang = self.transcribe_language.trim().to_ascii_lowercase();
        self.transcribe_language = if lang == "auto" || (2..=3).contains(&lang.len()) && lang.chars().all(|c| c.is_ascii_lowercase()) { lang } else { d.transcribe_language.clone() };
        for dir in [&mut self.recordings_dir, &mut self.screenshots_dir, &mut self.ffmpeg_path, &mut self.whisper_path, &mut self.whisper_model] {
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
        m.insert("recFps".to_string(), json!(25));
        m.insert("recEncoder".to_string(), json!("libx264; rm -rf /"));
        m.insert("recMaxMinutes".to_string(), json!(99999));
        m.insert("audioSampleRate".to_string(), json!(22050));
        m.insert("shotNameTemplate".to_string(), json!("../../etc/x"));
        m.insert("recSplitMinutes".to_string(), json!(7));
        let s = Settings::from_map(&m);
        assert_eq!(s.rec_fps, 30);
        assert_eq!(s.rec_encoder, "auto");
        assert_eq!(s.rec_max_minutes, 1440);
        assert_eq!(s.audio_sample_rate, 48000);
        assert_eq!(s.shot_name_template, "Screenshot {date} {time}");
        assert_eq!(s.rec_split_minutes, 0);
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

/// Expands a screenshot name template: {date} → 2026-11-12, {time} → 10-02-33, {mode} → screen.
pub fn expand_template(template: &str, date: &str, time: &str, mode: &str) -> String {
    template.replace("{date}", date).replace("{time}", time).replace("{mode}", mode)
}

#[cfg(test)]
mod template_tests {
    #[test]
    fn expands() {
        assert_eq!(super::expand_template("Shot {date} {time} {mode}", "2026-11-12", "10-00-00", "area"), "Shot 2026-11-12 10-00-00 area");
    }
}
