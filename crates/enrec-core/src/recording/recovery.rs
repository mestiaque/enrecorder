//! Recovery of recordings interrupted by a crash or power loss (phase 7).
//!
//! A recording is written as Matroska segments in `<folder>/.enrecorder-<session>/`. Matroska
//! written up to the moment the process died is readable but may lack its index and duration:
//! each part is re-muxed (no re-encoding) to repair it, unreadable parts are skipped, and the
//! rest are joined into "<name> (recovered).mkv" and added to the library.

use crate::database::{now_ms, Db, NewRecording};
use crate::error::{AppError, Result};
use crate::media::probe;
use crate::media::process::{run, tail};
use rusqlite::params;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Interrupted {
    pub session_id: String,
    /// "video" | "audio"
    pub kind: String,
    pub started_at: i64,
    /// The parts folder, when it still exists.
    pub folder: Option<String>,
    pub parts: usize,
    pub size_bytes: u64,
}

fn parts_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "mkv") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("part-"))).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Interrupted sessions whose parts can still be found in one of `folders`.
pub fn find(db: &Db, folders: &[PathBuf]) -> Result<Vec<Interrupted>> {
    let rows: Vec<(String, String, i64)> = {
        let c = db.conn.lock();
        let mut st = c.prepare("SELECT id, kind, started_at FROM capture_sessions WHERE state = 'interrupted' ORDER BY started_at DESC")?;
        let r = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        r
    };
    let mut out = Vec::new();
    for (id, kind, started_at) in rows {
        let dir = folders.iter().map(|f| f.join(format!(".enrecorder-{id}"))).find(|d| d.is_dir());
        let parts = dir.as_deref().map(parts_in).unwrap_or_default();
        let size = parts.iter().filter_map(|p| std::fs::metadata(p).ok()).map(|m| m.len()).sum();
        if parts.is_empty() {
            // Nothing left to save: close the session so it is not offered again.
            set_state(db, &id, "discarded")?;
            continue;
        }
        out.push(Interrupted { session_id: id, kind, started_at, folder: dir.map(|d| d.to_string_lossy().into_owned()), parts: parts.len(), size_bytes: size });
    }
    Ok(out)
}

fn set_state(db: &Db, id: &str, state: &str) -> Result<()> {
    db.conn.lock().execute("UPDATE capture_sessions SET state = ?2, ended_at = COALESCE(ended_at, ?3) WHERE id = ?1", params![id, state, now_ms()])?;
    Ok(())
}

/// Repairs and joins the parts; returns the recovered file and its library id.
pub fn recover(db: &Db, ffmpeg: &Path, ffprobe: Option<&Path>, item: &Interrupted, title: &str) -> Result<(PathBuf, String)> {
    let dir = PathBuf::from(item.folder.as_deref().ok_or_else(|| AppError::NotFound("the recorded parts are gone".into()))?);
    let out_dir = dir.parent().ok_or_else(|| AppError::Other("bad parts folder".into()))?.to_path_buf();
    let mut good = Vec::new();
    for (i, part) in parts_in(&dir).iter().enumerate() {
        let fixed = dir.join(format!("fixed-{:03}.mkv", i + 1));
        let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-err_detect", "ignore_err", "-fflags", "+genpts+discardcorrupt", "-i"]
            .iter()
            .map(|s| s.to_string())
            .chain([part.to_string_lossy().into_owned(), "-map".into(), "0".into(), "-c".into(), "copy".into(), "-f".into(), "matroska".into(), "-y".into(), fixed.to_string_lossy().into_owned()])
            .collect();
        let r = run(ffmpeg, &args, Duration::from_secs(600))?;
        let readable = std::fs::metadata(&fixed).map(|m| m.len() > 1024).unwrap_or(false)
            && ffprobe.is_none_or(|fp| probe::probe(fp, &fixed).is_ok_and(|p| (p.video_streams + p.audio_streams) > 0 && p.duration_ms.unwrap_or(0) > 0));
        if readable {
            good.push(fixed);
        } else {
            log::warn!("recovery: part {} is not readable ({})", part.display(), tail(&r.stderr, 1));
            let _ = std::fs::remove_file(&fixed);
        }
    }
    if good.is_empty() {
        return Err(AppError::Ffmpeg("none of the recorded parts could be read".into()));
    }
    let final_path = crate::recording::unique_path(&out_dir, &format!("{title} (recovered)"), "mkv");
    if good.len() == 1 {
        std::fs::rename(&good[0], &final_path)?;
    } else {
        let list = dir.join("recover.txt");
        std::fs::write(&list, good.iter().map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''"))).collect::<String>())?;
        let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-f", "concat", "-safe", "0", "-i"]
            .iter()
            .map(|s| s.to_string())
            .chain([list.to_string_lossy().into_owned(), "-map".into(), "0".into(), "-c".into(), "copy".into(), "-f".into(), "matroska".into(), "-y".into(), final_path.to_string_lossy().into_owned()])
            .collect();
        let r = run(ffmpeg, &args, Duration::from_secs(600))?;
        if !r.success {
            let _ = std::fs::remove_file(&final_path);
            return Err(AppError::Ffmpeg(format!("joining the parts failed: {}", tail(&r.stderr, 2))));
        }
    }
    let pr = ffprobe.and_then(|fp| probe::probe(fp, &final_path).ok());
    let size = std::fs::metadata(&final_path)?.len();
    let id = db.insert_recording(
        &NewRecording {
            kind: item.kind.clone(),
            title: final_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
            path: final_path.to_string_lossy().into_owned(),
            container: Some("mkv".into()),
            duration_ms: pr.as_ref().and_then(|p| p.duration_ms),
            size_bytes: size as i64,
            width: pr.as_ref().and_then(|p| p.width),
            height: pr.as_ref().and_then(|p| p.height),
            session_id: Some(item.session_id.clone()),
        },
        pr.as_ref(),
    )?;
    set_state(db, &item.session_id, "recovered")?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok((final_path, id))
}

/// Deletes the parts and closes the session.
pub fn discard(db: &Db, item: &Interrupted) -> Result<()> {
    if let Some(f) = &item.folder {
        let dir = Path::new(f);
        // Only ever a parts folder of ours.
        if dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".enrecorder-")) {
            std::fs::remove_dir_all(dir)?;
        }
    }
    set_state(db, &item.session_id, "discarded")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::NewSession;
    use crate::media::ffmpeg::find_tool;

    /// Two parts as a crash leaves them: one complete, one cut off mid-file (truncated), plus
    /// junk. Recovery repairs and joins what can be read.
    #[test]
    fn recovers_parts_left_by_a_crash() {
        let (Some(ff), Some(fp)) = (find_tool("ffmpeg", None, &[]), find_tool("ffprobe", None, &[])) else { return };
        let (ff, fp) = (PathBuf::from(ff.path), PathBuf::from(fp.path));
        let root = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        db.insert_session(&NewSession { id: "s1".into(), kind: "video".into(), state: "recording".into(), source: "{}".into(), settings: "{}".into(), output_path: None }).unwrap();
        db.insert_session(&NewSession { id: "s2".into(), kind: "video".into(), state: "recording".into(), source: "{}".into(), settings: "{}".into(), output_path: None }).unwrap();
        assert_eq!(db.mark_interrupted_sessions().unwrap(), 2);
        let dir = root.path().join(".enrecorder-s1");
        std::fs::create_dir_all(&dir).unwrap();
        for (n, secs) in [(1, 2), (2, 3)] {
            let p = dir.join(format!("part-{n:03}.mkv"));
            assert!(crate::media::process::command(&ff).args(["-loglevel", "error", "-f", "lavfi", "-i", &format!("testsrc2=size=320x240:rate=25:duration={secs}"), "-c:v", "libx264", "-f", "matroska", "-y"]).arg(&p).status().unwrap().success());
        }
        // Cut the second part as if the process died while writing it.
        let p2 = dir.join("part-002.mkv");
        let len = std::fs::metadata(&p2).unwrap().len();
        std::fs::OpenOptions::new().write(true).open(&p2).unwrap().set_len(len * 2 / 3).unwrap();
        std::fs::write(dir.join("part-003.mkv"), b"garbage").unwrap();

        let found = find(&db, &[root.path().to_path_buf()]).unwrap();
        assert_eq!(found.len(), 1, "s2 has no parts and is closed");
        assert_eq!(found[0].parts, 3);
        let (path, _) = recover(&db, &ff, Some(&fp), &found[0], "Recording").unwrap();
        assert_eq!(path.file_name().unwrap(), "Recording (recovered).mkv");
        let d = probe::probe(&fp, &path).unwrap().duration_ms.unwrap();
        assert!((2500..5200).contains(&d), "recovered duration {d}");
        assert!(!dir.exists());
        assert_eq!(db.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'recovered'").unwrap(), Some(1));
        assert!(find(&db, &[root.path().to_path_buf()]).unwrap().is_empty());
    }
}
