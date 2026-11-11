//! Screenshots: one frame from the same capture backends as video (FFmpeg reads the screen),
//! the Wayland screenshot portal, image conversion, and the library rows.
//!
//! The captured image is kept untouched as the *original* (hidden folder next to the
//! screenshots) so annotations stay editable; the visible file is the flattened export the
//! editor writes (PNG, JPEG or WebP).

use crate::capture::CaptureInput;
use crate::database::{new_id, now_ms, Db};
use crate::error::{AppError, Result};
use crate::media::process::{run, tail};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FORMATS: &[&str] = &["png", "jpeg", "webp"];

/// Folder for the untouched captures (annotations are re-applied to these).
pub fn originals_dir(screenshots: &Path) -> PathBuf {
    screenshots.join(".enrecorder-originals")
}

fn s(v: &str) -> String {
    v.to_string()
}

/// Width × height of a PNG from its header (no decoder needed).
pub fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

/// Grabs one frame from a capture input into `out` (PNG).
pub fn grab(ffmpeg: &Path, input: &CaptureInput, out: &Path) -> Result<(u32, u32)> {
    if input.frames.is_some() {
        return Err(AppError::Unsupported("this capture method delivers frames itself; use the screenshot portal".into()));
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut args = vec![s("-hide_banner"), s("-loglevel"), s("error"), s("-nostdin")];
    args.extend(input.args.iter().cloned());
    args.extend([s("-frames:v"), s("1"), s("-update"), s("1"), s("-f"), s("image2"), s("-c:v"), s("png"), s("-y"), out.to_string_lossy().into_owned()]);
    let r = run(ffmpeg, &args, Duration::from_secs(20))?;
    if !r.success {
        return Err(AppError::Ffmpeg(format!("the screen could not be captured: {}", tail(&r.stderr, 3))));
    }
    let bytes = std::fs::read(out)?;
    png_size(&bytes).ok_or_else(|| AppError::Ffmpeg("the captured image is not a valid PNG".into()))
}

/// Writes `png` (the editor's flattened image) to `out` in `format`. JPEG and WebP are encoded
/// by FFmpeg, so no image codec has to be bundled.
pub fn export(ffmpeg: &Path, png: &[u8], format: &str, quality: u32, out: &Path) -> Result<u64> {
    if !FORMATS.contains(&format) {
        return Err(AppError::InvalidSetting(format!("unknown image format {format}")));
    }
    png_size(png).ok_or_else(|| AppError::InvalidSetting("the image to save is not a PNG".into()))?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = out.with_extension(format!("tmp.{}", ext(format)));
    if format == "png" {
        std::fs::write(&tmp, png)?;
    } else {
        let src = out.with_extension("src.png");
        std::fs::write(&src, png)?;
        let q = quality.clamp(1, 100);
        let mut args = vec![s("-hide_banner"), s("-loglevel"), s("error"), s("-nostdin"), s("-i"), src.to_string_lossy().into_owned(), s("-frames:v"), s("1")];
        if format == "jpeg" {
            // FFmpeg's mjpeg scale: 2 (best) … 31 (worst).
            let qv = 2 + (100 - q) * 29 / 100;
            args.extend([s("-c:v"), s("mjpeg"), s("-q:v"), qv.to_string(), s("-pix_fmt"), s("yuvj444p")]);
        } else {
            args.extend([s("-c:v"), s("libwebp"), s("-quality"), q.to_string()]);
        }
        args.extend([s("-f"), s("image2"), s("-y"), tmp.to_string_lossy().into_owned()]);
        let r = run(ffmpeg, &args, Duration::from_secs(30));
        let _ = std::fs::remove_file(&src);
        let r = r?;
        if !r.success {
            let _ = std::fs::remove_file(&tmp);
            return Err(AppError::Ffmpeg(format!("the image could not be saved as {}: {}", format.to_uppercase(), tail(&r.stderr, 2))));
        }
    }
    std::fs::rename(&tmp, out)?;
    Ok(std::fs::metadata(out)?.len())
}

pub fn ext(format: &str) -> &'static str {
    match format {
        "jpeg" => "jpg",
        "webp" => "webp",
        _ => "png",
    }
}

// ------------------------------------------------------------------ library

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub id: String,
    pub title: String,
    pub path: String,
    pub format: String,
    pub width: i64,
    pub height: i64,
    pub size_bytes: i64,
    /// Editor document (JSON) applied to the original.
    pub annotations: Option<String>,
    pub created_at: i64,
}

impl Db {
    pub fn insert_screenshot(&self, title: &str, path: &str, format: &str, width: u32, height: u32, size: u64) -> Result<String> {
        let id = new_id();
        let now = now_ms();
        self.conn.lock().execute(
            "INSERT INTO screenshots(id, title, path, format, width, height, size_bytes, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ready', ?8, ?8)",
            params![id, title, path, format, width, height, size as i64, now],
        )?;
        Ok(id)
    }

    /// After editing: the new flattened file, its format/size, and the annotation document.
    #[allow(clippy::too_many_arguments)]
    pub fn update_screenshot(&self, id: &str, path: &str, format: &str, width: u32, height: u32, size: u64, annotations: Option<&str>) -> Result<()> {
        let n = self.conn.lock().execute(
            "UPDATE screenshots SET path = ?2, format = ?3, width = ?4, height = ?5, size_bytes = ?6, annotations = ?7, updated_at = ?8 WHERE id = ?1",
            params![id, path, format, width, height, size as i64, annotations, now_ms()],
        )?;
        if n == 0 {
            return Err(AppError::NotFound(format!("screenshot {id}")));
        }
        Ok(())
    }

    pub fn screenshot(&self, id: &str) -> Result<Screenshot> {
        self.conn
            .lock()
            .query_row("SELECT id, title, path, format, width, height, size_bytes, annotations, created_at FROM screenshots WHERE id = ?1", params![id], |r| {
                Ok(Screenshot { id: r.get(0)?, title: r.get(1)?, path: r.get(2)?, format: r.get(3)?, width: r.get(4)?, height: r.get(5)?, size_bytes: r.get(6)?, annotations: r.get(7)?, created_at: r.get(8)? })
            })
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("screenshot {id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::find_tool;

    #[test]
    fn png_header() {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend(640u32.to_be_bytes());
        b.extend(480u32.to_be_bytes());
        assert_eq!(png_size(&b), Some((640, 480)));
        assert_eq!(png_size(b"GIF89a"), None);
    }

    #[test]
    fn grab_export_and_library() {
        let Some(ff) = find_tool("ffmpeg", None, &[]) else { return };
        let ff = PathBuf::from(ff.path);
        let dir = tempfile::tempdir().unwrap();
        // A synthetic source stands in for the screen.
        let input = CaptureInput { args: ["-f", "lavfi", "-i", "testsrc2=size=320x240:rate=1"].iter().map(|s| s.to_string()).collect(), width: 320, height: 240, frames: None };
        let orig = originals_dir(dir.path()).join("a.png");
        assert_eq!(grab(&ff, &input, &orig).unwrap(), (320, 240));
        let png = std::fs::read(&orig).unwrap();
        for f in FORMATS {
            let out = dir.path().join(format!("shot.{}", ext(f)));
            let size = export(&ff, &png, f, 90, &out).unwrap();
            assert!(size > 100, "{f}: {size}");
        }
        assert!(export(&ff, b"nope", "png", 90, &dir.path().join("x.png")).is_err());
        assert!(export(&ff, &png, "gif", 90, &dir.path().join("x.gif")).is_err());
        let db = Db::open_in_memory().unwrap();
        let id = db.insert_screenshot("Shot", "/s/a.png", "png", 320, 240, 10).unwrap();
        db.update_screenshot(&id, "/s/a.webp", "webp", 300, 200, 20, Some("{\"items\":[]}")).unwrap();
        let s = db.screenshot(&id).unwrap();
        assert_eq!((s.format.as_str(), s.width, s.annotations.as_deref()), ("webp", 300, Some("{\"items\":[]}")));
        assert!(db.update_screenshot("missing", "/x", "png", 1, 1, 1, None).is_err());
    }
}
