//! Library thumbnails (JPEG, 320 px wide), made once and cached: a frame from a video, a
//! waveform for audio, a scaled copy for screenshots.

use crate::error::{AppError, Result};
use crate::media::process::{run, tail};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn path_for(cache: &Path, id: &str) -> PathBuf {
    cache.join(format!("{id}.jpg"))
}

/// Returns the cached thumbnail, making it first when needed. `kind`: video | audio | screenshot.
pub fn thumbnail(ffmpeg: &Path, cache: &Path, id: &str, kind: &str, source: &Path, duration_ms: Option<i64>) -> Result<PathBuf> {
    let out = path_for(cache, id);
    let fresh = |o: &Path| match (std::fs::metadata(o).and_then(|m| m.modified()), std::fs::metadata(source).and_then(|m| m.modified())) {
        (Ok(t), Ok(s)) => t >= s,
        _ => false,
    };
    if out.is_file() && fresh(&out) {
        return Ok(out);
    }
    if !source.is_file() {
        return Err(AppError::NotFound(format!("{} is missing", source.display())));
    }
    std::fs::create_dir_all(cache)?;
    let tmp = out.with_extension("tmp.jpg");
    let src = source.to_string_lossy().into_owned();
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"].iter().map(|s| s.to_string()).collect();
    match kind {
        "video" => {
            // A frame a little into the recording (not the first, often black, frame).
            let at = duration_ms.map_or(1.0, |d| (d as f64 / 1000.0 * 0.1).min(5.0));
            a.extend(["-ss".into(), format!("{at:.2}"), "-i".into(), src, "-frames:v".into(), "1".into(), "-vf".into(), "scale=320:-2".into()]);
        }
        "audio" => a.extend(["-i".into(), src, "-filter_complex".into(), "aformat=channel_layouts=mono,showwavespic=s=320x180:colors=0x8b8cf8".into(), "-frames:v".into(), "1".into()]),
        _ => a.extend(["-i".into(), src, "-frames:v".into(), "1".into(), "-vf".into(), "scale=320:-2".into()]),
    }
    a.extend(["-q:v".into(), "4".into(), "-f".into(), "image2".into(), "-y".into(), tmp.to_string_lossy().into_owned()]);
    let r = run(ffmpeg, &a, Duration::from_secs(30))?;
    if !r.success || !tmp.is_file() {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::Ffmpeg(format!("no thumbnail: {}", tail(&r.stderr, 2))));
    }
    std::fs::rename(&tmp, &out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::find_tool;

    #[test]
    fn makes_and_reuses_thumbnails() {
        let Some(ff) = find_tool("ffmpeg", None, &[]) else { return };
        let ff = PathBuf::from(ff.path);
        let dir = tempfile::tempdir().unwrap();
        let v = dir.path().join("v.mkv");
        let a = dir.path().join("a.wav");
        let mk = |args: &[&str], out: &Path| assert!(crate::media::process::command(&ff).args(args).arg(out).status().unwrap().success());
        mk(&["-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:duration=2", "-y"], &v);
        mk(&["-loglevel", "error", "-f", "lavfi", "-i", "sine=duration=2", "-y"], &a);
        let cache = dir.path().join("thumbs");
        let t1 = thumbnail(&ff, &cache, "v1", "video", &v, Some(2000)).unwrap();
        let t2 = thumbnail(&ff, &cache, "a1", "audio", &a, Some(2000)).unwrap();
        assert!(std::fs::metadata(&t1).unwrap().len() > 500 && std::fs::metadata(&t2).unwrap().len() > 200);
        let before = std::fs::metadata(&t1).unwrap().modified().unwrap();
        thumbnail(&ff, &cache, "v1", "video", &v, Some(2000)).unwrap();
        assert_eq!(std::fs::metadata(&t1).unwrap().modified().unwrap(), before, "cached");
        assert!(thumbnail(&ff, &cache, "x", "video", &dir.path().join("missing.mp4"), None).is_err());
    }
}
