//! Text from screenshots (phase 6) with the Tesseract OCR engine (a separate program: the
//! `tesseract-ocr` package on Ubuntu, the UB Mannheim installer on Windows). Nothing leaves the
//! computer.

use crate::error::{AppError, Result};
use crate::media::process::{run, tail};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn find_tesseract() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "tesseract.exe" } else { "tesseract" };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(b) = std::env::var_os(base) {
                dirs.push(PathBuf::from(&b).join("Tesseract-OCR"));
                dirs.push(PathBuf::from(b).join("Programs").join("Tesseract-OCR"));
            }
        }
    }
    dirs.into_iter().map(|d| d.join(exe)).find(|p| p.is_file())
}

/// Installed language packs ("eng", "ben", …), without the "osd" helper.
pub fn languages(tess: &Path) -> Vec<String> {
    run(tess, &["--list-langs".to_string()], Duration::from_secs(10))
        .map(|r| r.stdout.lines().skip(1).map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && l != "osd").collect())
        .unwrap_or_default()
}

/// Recognises the text in a PNG. `langs` like "eng+ben" (only installed ones are used).
pub fn recognize(tess: &Path, png: &Path, langs: &str) -> Result<String> {
    let installed = languages(tess);
    let wanted: Vec<&str> = langs.split('+').filter(|l| installed.iter().any(|i| i == l)).collect();
    let lang = if wanted.is_empty() { installed.first().cloned().unwrap_or_else(|| "eng".into()) } else { wanted.join("+") };
    let args = vec![png.to_string_lossy().into_owned(), "stdout".into(), "-l".into(), lang, "--psm".into(), "3".into()];
    let r = run(tess, &args, Duration::from_secs(120))?;
    if !r.success {
        return Err(AppError::Other(format!("text recognition failed: {}", tail(&r.stderr, 2))));
    }
    // Tidy: trailing spaces, form feeds, runs of blank lines.
    let mut out = String::new();
    let mut blank = 0;
    for line in r.stdout.replace('\u{c}', "").lines() {
        let l = line.trim_end();
        if l.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(l);
        out.push('\n');
    }
    Ok(out.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::find_tool;

    /// Real Tesseract (skipped when not installed): FFmpeg draws a line of text, OCR reads it.
    #[test]
    fn reads_text_from_an_image() {
        let (Some(t), Some(ff)) = (find_tesseract(), find_tool("ffmpeg", None, &[])) else { return };
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("t.png");
        let font = ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"].into_iter().find(|f| Path::new(f).is_file());
        let Some(font) = font else { return };
        let draw = format!("drawtext=fontfile={font}:text='Hello ENRecorder 2026':fontsize=48:fontcolor=black:x=20:y=30");
        let ok = crate::media::process::command(Path::new(&ff.path))
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "color=white:size=800x120", "-vf", &draw, "-frames:v", "1", "-y"])
            .arg(&png)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let text = recognize(&t, &png, "eng").unwrap();
        assert!(text.contains("Hello") && text.contains("2026"), "{text}");
    }
}
