//! Export queue: convert, compress, extract audio, GIF and trim jobs, one FFmpeg process at a
//! time, persisted in `export_jobs`, with progress from FFmpeg's `-progress` output and cancel.
//! Finished files are verified with FFprobe and added to the library.

use crate::database::{new_id, now_ms, Db, NewRecording};
use crate::error::{AppError, Result};
use crate::media::probe;
use crate::media::process::command;
use parking_lot::Mutex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportPreset {
    /// "convert" | "compress" | "audio" | "gif" | "trim"
    pub kind: String,
    /// Output format: mp4 | webm | mkv | mp3 | m4a | wav | flac | gif
    pub format: String,
    /// "high" | "balanced" | "small"
    pub quality: String,
    /// Trim range in milliseconds (any kind may use it).
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    /// Scale down to this height (never up); 0 = keep.
    pub max_height: u32,
    pub remove_audio: bool,
    /// Audio volume in percent (100 = unchanged).
    pub volume: u32,
}

impl Default for ExportPreset {
    fn default() -> Self {
        Self { kind: "convert".into(), format: "mp4".into(), quality: "balanced".into(), start_ms: None, end_ms: None, max_height: 0, remove_audio: false, volume: 100 }
    }
}

const VIDEO: &[&str] = &["mp4", "webm", "mkv", "gif"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac", "opus"];

fn s(v: &str) -> String {
    v.to_string()
}

fn secs(ms: u64) -> String {
    format!("{:.3}", ms as f64 / 1000.0)
}

/// FFmpeg arguments (after `-progress`), and the length of the output in ms for progress.
pub fn build_args(p: &ExportPreset, src: &Path, dst: &Path, source_ms: Option<u64>, has_video: bool) -> Result<(Vec<String>, Option<u64>)> {
    if !matches!(p.kind.as_str(), "convert" | "compress" | "audio" | "gif" | "trim") {
        return Err(AppError::InvalidSetting(format!("unknown export {}", p.kind)));
    }
    let audio_only_out = AUDIO.contains(&p.format.as_str());
    if !audio_only_out && !VIDEO.contains(&p.format.as_str()) {
        return Err(AppError::InvalidSetting(format!("unknown format {}", p.format)));
    }
    if !has_video && !audio_only_out {
        return Err(AppError::InvalidSetting("this recording has no video; choose an audio format".into()));
    }
    if let (Some(a), Some(b)) = (p.start_ms, p.end_ms) {
        if b <= a + 100 {
            return Err(AppError::InvalidSetting("the end must be after the start".into()));
        }
    }
    let q = p.quality.as_str();
    let pick = |hi: &str, mid: &str, lo: &str| s(match q { "high" => hi, "small" => lo, _ => mid });
    let mut a = vec![s("-hide_banner"), s("-loglevel"), s("error"), s("-nostats"), s("-progress"), s("pipe:1")];
    if let Some(st) = p.start_ms.filter(|v| *v > 0) {
        a.extend([s("-ss"), secs(st)]);
    }
    if let Some(en) = p.end_ms {
        a.extend([s("-to"), secs(en)]);
    }
    a.extend([s("-i"), src.to_string_lossy().into_owned()]);
    let out_ms = match (p.start_ms, p.end_ms) {
        (st, Some(en)) => Some(en.saturating_sub(st.unwrap_or(0))),
        (Some(st), None) => source_ms.map(|d| d.saturating_sub(st)),
        _ => source_ms,
    };
    let scale = if p.max_height > 0 { format!("scale=-2:'min({},ih)'", p.max_height) } else { s("scale=trunc(iw/2)*2:trunc(ih/2)*2") };
    let vol = (p.volume != 100).then(|| format!("volume={:.2}", p.volume.min(400) as f64 / 100.0));
    let mut af: Vec<String> = Vec::new();
    if let Some(v) = &vol {
        af.push(v.clone());
    }
    match p.format.as_str() {
        "gif" => {
            let h = if p.max_height > 0 { p.max_height } else { 480 };
            let fps = pick("15", "12", "10");
            a.extend([s("-filter_complex"), format!("[0:v]fps={fps},scale=-2:'min({h},ih)':flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"), s("-an"), s("-loop"), s("0")]);
        }
        "mp4" | "mkv" => {
            a.extend([s("-map"), s("0:v:0"), s("-vf"), scale, s("-c:v"), s("libx264"), s("-preset"), s(if p.kind == "compress" { "medium" } else { "veryfast" }), s("-pix_fmt"), s("yuv420p")]);
            let crf = if p.kind == "compress" { pick("24", "28", "32") } else { pick("18", "22", "26") };
            a.extend([s("-crf"), crf]);
            if p.remove_audio {
                a.push(s("-an"));
            } else {
                a.extend([s("-map"), s("0:a?"), s("-c:a"), s("aac"), s("-b:a"), if p.kind == "compress" { s("96k") } else { pick("192k", "160k", "128k") }]);
                if !af.is_empty() {
                    a.extend([s("-af"), af.join(",")]);
                }
            }
            if p.format == "mp4" {
                a.extend([s("-movflags"), s("+faststart")]);
            }
        }
        "webm" => {
            a.extend([s("-map"), s("0:v:0"), s("-vf"), scale, s("-c:v"), s("libvpx-vp9"), s("-deadline"), s("good"), s("-cpu-used"), s("4"), s("-row-mt"), s("1"), s("-b:v"), s("0"), s("-crf"), pick("28", "33", "38")]);
            if p.remove_audio {
                a.push(s("-an"));
            } else {
                a.extend([s("-map"), s("0:a?"), s("-c:a"), s("libopus"), s("-b:a"), s("128k")]);
                if !af.is_empty() {
                    a.extend([s("-af"), af.join(",")]);
                }
            }
        }
        f => {
            // Audio formats: the first audio track (all tracks mixed for multi-track sources).
            a.push(s("-vn"));
            a.extend([s("-map"), s("0:a:0")]);
            if !af.is_empty() {
                a.extend([s("-af"), af.join(",")]);
            }
            match f {
                "mp3" => a.extend([s("-c:a"), s("libmp3lame"), s("-b:a"), pick("256k", "192k", "128k")]),
                "m4a" => a.extend([s("-c:a"), s("aac"), s("-b:a"), pick("256k", "160k", "96k")]),
                "opus" => a.extend([s("-c:a"), s("libopus"), s("-b:a"), pick("160k", "112k", "64k")]),
                "flac" => a.extend([s("-c:a"), s("flac")]),
                _ => a.extend([s("-c:a"), s("pcm_s16le")]),
            }
        }
    }
    a.extend([s("-y"), dst.to_string_lossy().into_owned()]);
    Ok((a, out_ms))
}

pub fn extension(format: &str) -> &'static str {
    match format {
        "webm" => "webm",
        "mkv" => "mkv",
        "gif" => "gif",
        "mp3" => "mp3",
        "m4a" => "m4a",
        "wav" => "wav",
        "flac" => "flac",
        "opus" => "opus",
        _ => "mp4",
    }
}

// ------------------------------------------------------------------ queue

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportUpdate {
    pub id: String,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
    pub output_path: String,
}

pub trait ExportObserver: Send + Sync {
    fn update(&self, u: &ExportUpdate);
}

struct Job {
    id: String,
    src: PathBuf,
    dst: PathBuf,
    preset: ExportPreset,
    source_ms: Option<u64>,
    has_video: bool,
}

pub struct ExportManager {
    db: Arc<Db>,
    ffmpeg: Mutex<Option<PathBuf>>,
    ffprobe: Mutex<Option<PathBuf>>,
    queue: Mutex<VecDeque<Job>>,
    running: Mutex<Option<(String, Arc<AtomicBool>)>>,
    worker: Mutex<bool>,
    observer: Arc<dyn ExportObserver>,
}

impl ExportManager {
    pub fn new(db: Arc<Db>, observer: Arc<dyn ExportObserver>) -> Arc<Self> {
        Arc::new(Self { db, ffmpeg: Mutex::new(None), ffprobe: Mutex::new(None), queue: Mutex::new(VecDeque::new()), running: Mutex::new(None), worker: Mutex::new(false), observer })
    }

    pub fn set_tools(&self, ffmpeg: Option<PathBuf>, ffprobe: Option<PathBuf>) {
        *self.ffmpeg.lock() = ffmpeg;
        *self.ffprobe.lock() = ffprobe;
    }

    /// Adds a job; the output gets a free name next to `out_dir`. Returns the job id.
    #[allow(clippy::too_many_arguments)]
    pub fn enqueue(self: &Arc<Self>, source_id: Option<&str>, src: &Path, out_dir: &Path, stem: &str, preset: ExportPreset, source_ms: Option<u64>, has_video: bool) -> Result<String> {
        if self.ffmpeg.lock().is_none() {
            return Err(AppError::FfmpegMissing);
        }
        if !src.is_file() {
            return Err(AppError::NotFound(format!("{} is missing", src.display())));
        }
        std::fs::create_dir_all(out_dir)?;
        // Validate now so a bad preset never reaches the queue.
        let dst = crate::recording::unique_path(out_dir, stem, extension(&preset.format));
        build_args(&preset, src, &dst, source_ms, has_video)?;
        let id = new_id();
        self.db.conn.lock().execute(
            "INSERT INTO export_jobs(id, source_id, source_path, output_path, preset, status, progress, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'queued', 0, ?6)",
            params![id, source_id, src.to_string_lossy(), dst.to_string_lossy(), serde_json::to_string(&preset)?, now_ms()],
        )?;
        self.notify(&id, "queued", 0.0, None, &dst);
        self.queue.lock().push_back(Job { id: id.clone(), src: src.to_path_buf(), dst, preset, source_ms, has_video });
        self.ensure_worker();
        Ok(id)
    }

    /// Cancels a queued or running job.
    pub fn cancel(&self, id: &str) -> Result<()> {
        if let Some((rid, stop)) = self.running.lock().as_ref() {
            if rid == id {
                stop.store(true, Ordering::Release);
                return Ok(());
            }
        }
        let mut q = self.queue.lock();
        if let Some(i) = q.iter().position(|j| j.id == id) {
            let j = q.remove(i).unwrap();
            drop(q);
            self.finish(&j.id, "cancelled", None, &j.dst);
            return Ok(());
        }
        Err(AppError::NotFound(format!("export {id} is not waiting or running")))
    }

    /// Jobs left "queued"/"running" by a previous run are marked failed (their files are partial).
    pub fn recover(&self) -> Result<usize> {
        Ok(self.db.conn.lock().execute("UPDATE export_jobs SET status = 'failed', error = 'ENRecorder was closed during the export', finished_at = ?1 WHERE status IN ('queued', 'initializing', 'running', 'finalizing')", params![now_ms()])?)
    }

    fn notify(&self, id: &str, status: &str, progress: f64, error: Option<String>, dst: &Path) {
        self.observer.update(&ExportUpdate { id: id.into(), status: status.into(), progress, error, output_path: dst.to_string_lossy().into_owned() });
    }

    fn set(&self, id: &str, status: &str, progress: f64) {
        let started = if status == "running" { Some(now_ms()) } else { None };
        let _ = self.db.conn.lock().execute("UPDATE export_jobs SET status = ?2, progress = ?3, started_at = COALESCE(?4, started_at) WHERE id = ?1", params![id, status, progress, started]);
    }

    fn finish(&self, id: &str, status: &str, error: Option<String>, dst: &Path) {
        let progress = if status == "completed" { 1.0 } else { 0.0 };
        let _ = self.db.conn.lock().execute("UPDATE export_jobs SET status = ?2, progress = ?3, error = ?4, finished_at = ?5 WHERE id = ?1", params![id, status, progress, error, now_ms()]);
        self.notify(id, status, progress, error, dst);
    }

    fn ensure_worker(self: &Arc<Self>) {
        let mut w = self.worker.lock();
        if *w {
            return;
        }
        *w = true;
        let me = self.clone();
        std::thread::Builder::new()
            .name("export-queue".into())
            .spawn(move || loop {
                let job = me.queue.lock().pop_front();
                let Some(job) = job else {
                    *me.worker.lock() = false;
                    return;
                };
                me.run(job);
            })
            .ok();
    }

    fn run(&self, job: Job) {
        let stop = Arc::new(AtomicBool::new(false));
        *self.running.lock() = Some((job.id.clone(), stop.clone()));
        let result = self.execute(&job, &stop);
        *self.running.lock() = None;
        match result {
            Ok(()) => self.finish(&job.id, "completed", None, &job.dst),
            Err(e) => {
                let _ = std::fs::remove_file(&job.dst);
                let cancelled = stop.load(Ordering::Acquire);
                self.finish(&job.id, if cancelled { "cancelled" } else { "failed" }, (!cancelled).then(|| e.to_string()), &job.dst);
            }
        }
    }

    fn execute(&self, job: &Job, stop: &AtomicBool) -> Result<()> {
        let ffmpeg = self.ffmpeg.lock().clone().ok_or(AppError::FfmpegMissing)?;
        let (args, out_ms) = build_args(&job.preset, &job.src, &job.dst, job.source_ms, job.has_video)?;
        self.set(&job.id, "running", 0.0);
        self.notify(&job.id, "running", 0.0, None, &job.dst);
        log::info!("export {}: ffmpeg {}", job.id, args.join(" "));
        let mut child = command(&ffmpeg).args(&args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| AppError::Ffmpeg(format!("could not start FFmpeg: {e}")))?;
        let stderr = Arc::new(Mutex::new(String::new()));
        if let Some(mut e) = child.stderr.take() {
            let buf = stderr.clone();
            std::thread::spawn(move || {
                let mut t = String::new();
                let _ = e.read_to_string(&mut t);
                *buf.lock() = t;
            });
        }
        let out = child.stdout.take();
        let mut stdin = child.stdin.take();
        let mut last = Instant::now();
        if let Some(out) = out {
            for line in BufReader::new(out).lines().map_while(std::result::Result::ok) {
                if stop.load(Ordering::Acquire) {
                    if let Some(si) = stdin.as_mut() {
                        let _ = si.write_all(b"q\n");
                        let _ = si.flush();
                    }
                    let _ = child.kill();
                    break;
                }
                if let Some(v) = line.strip_prefix("out_time_us=").or_else(|| line.strip_prefix("out_time_ms=")) {
                    if let (Ok(us), Some(total)) = (v.trim().parse::<u64>(), out_ms.filter(|t| *t > 0)) {
                        let p = (us as f64 / 1000.0 / total as f64).clamp(0.0, 0.99);
                        if last.elapsed() >= Duration::from_millis(250) {
                            last = Instant::now();
                            self.set(&job.id, "running", p);
                            self.notify(&job.id, "running", p, None, &job.dst);
                        }
                    }
                }
            }
        }
        let status = child.wait()?;
        if stop.load(Ordering::Acquire) {
            return Err(AppError::Other("cancelled".into()));
        }
        if !status.success() {
            let err = stderr.lock().clone();
            return Err(AppError::Ffmpeg(crate::media::process::tail(&err, 3)));
        }
        // Verify and add to the library.
        let size = std::fs::metadata(&job.dst).map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            return Err(AppError::Ffmpeg("the exported file is empty".into()));
        }
        let pr = self.ffprobe.lock().clone().and_then(|fp| probe::probe(&fp, &job.dst).ok());
        let audio = AUDIO.contains(&job.preset.format.as_str());
        let title = job.dst.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        self.db.insert_recording(
            &NewRecording {
                kind: if audio { "audio".into() } else { "video".into() },
                title,
                path: job.dst.to_string_lossy().into_owned(),
                container: Some(extension(&job.preset.format).into()),
                duration_ms: pr.as_ref().and_then(|p| p.duration_ms).or(out_ms.map(|m| m as i64)),
                size_bytes: size as i64,
                width: pr.as_ref().and_then(|p| p.width),
                height: pr.as_ref().and_then(|p| p.height),
                session_id: None,
            },
            pr.as_ref(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::find_tool;
    use std::sync::mpsc;

    #[test]
    fn arguments() {
        let src = Path::new("/in.mkv");
        let dst = Path::new("/out.mp4");
        let trim = ExportPreset { kind: "trim".into(), start_ms: Some(2000), end_ms: Some(5000), ..Default::default() };
        let (a, ms) = build_args(&trim, src, dst, Some(10_000), true).unwrap();
        assert_eq!(ms, Some(3000));
        assert_eq!(&a[6..10], &["-ss", "2.000", "-to", "5.000"]);
        assert!(build_args(&ExportPreset { format: "mp4".into(), ..Default::default() }, src, dst, None, false).is_err(), "no video → no MP4");
        assert!(build_args(&ExportPreset { start_ms: Some(5000), end_ms: Some(5000), ..Default::default() }, src, dst, None, true).is_err());
        assert!(build_args(&ExportPreset { format: "avi".into(), ..Default::default() }, src, dst, None, true).is_err());
        let gif = build_args(&ExportPreset { kind: "gif".into(), format: "gif".into(), ..Default::default() }, src, dst, Some(1000), true).unwrap().0;
        assert!(gif.iter().any(|x| x.contains("palettegen")));
        let mp3 = build_args(&ExportPreset { kind: "audio".into(), format: "mp3".into(), volume: 150, ..Default::default() }, src, dst, Some(1000), false).unwrap().0;
        assert!(mp3.contains(&"-vn".to_string()) && mp3.contains(&"volume=1.50".to_string()));
    }

    struct Chan(Mutex<mpsc::Sender<ExportUpdate>>);
    impl ExportObserver for Chan {
        fn update(&self, u: &ExportUpdate) {
            let _ = self.0.lock().send(u.clone());
        }
    }

    fn wait_done(rx: &mpsc::Receiver<ExportUpdate>, id: &str) -> ExportUpdate {
        let end = Instant::now() + Duration::from_secs(60);
        while Instant::now() < end {
            if let Ok(u) = rx.recv_timeout(Duration::from_secs(1)) {
                if u.id == id && matches!(u.status.as_str(), "completed" | "failed" | "cancelled") {
                    return u;
                }
            }
        }
        panic!("export {id} did not finish");
    }

    /// Real FFmpeg: a 4 s clip → trimmed MP4 (1 s–3 s), GIF and MP3; library rows written.
    #[test]
    fn queue_runs_real_exports() {
        let (Some(ff), Some(fp)) = (find_tool("ffmpeg", None, &[]), find_tool("ffprobe", None, &[])) else { return };
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("clip.mkv");
        let ok = command(Path::new(&ff.path))
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=320x240:rate=25:duration=4", "-f", "lavfi", "-i", "sine=duration=4", "-c:v", "libx264", "-c:a", "aac", "-y"])
            .arg(&src)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let db = Arc::new(Db::open_in_memory().unwrap());
        let (tx, rx) = mpsc::channel();
        let m = ExportManager::new(db.clone(), Arc::new(Chan(Mutex::new(tx))));
        m.set_tools(Some(PathBuf::from(&ff.path)), Some(PathBuf::from(&fp.path)));
        let out = dir.path().join("Exports");
        let trim = m.enqueue(None, &src, &out, "clip (trim)", ExportPreset { kind: "trim".into(), start_ms: Some(1000), end_ms: Some(3000), ..Default::default() }, Some(4000), true).unwrap();
        let gif = m.enqueue(None, &src, &out, "clip", ExportPreset { kind: "gif".into(), format: "gif".into(), max_height: 120, ..Default::default() }, Some(4000), true).unwrap();
        let mp3 = m.enqueue(None, &src, &out, "clip", ExportPreset { kind: "audio".into(), format: "mp3".into(), ..Default::default() }, Some(4000), true).unwrap();
        for id in [&trim, &gif, &mp3] {
            let u = wait_done(&rx, id);
            assert_eq!(u.status, "completed", "{:?}", u.error);
        }
        let t = probe::probe(Path::new(&fp.path), &out.join("clip (trim).mp4")).unwrap();
        let d = t.duration_ms.unwrap();
        assert!((1800..2300).contains(&d), "trim duration {d}");
        assert!(out.join("clip.gif").is_file() && out.join("clip.mp3").is_file());
        assert_eq!(db.query_i64("SELECT COUNT(*) FROM recordings").unwrap(), Some(3));
        assert_eq!(db.query_i64("SELECT COUNT(*) FROM export_jobs WHERE status = 'completed'").unwrap(), Some(3));
        // Cancel a queued job.
        let a = m.enqueue(None, &src, &out, "slow", ExportPreset { kind: "compress".into(), ..Default::default() }, Some(4000), true).unwrap();
        let b = m.enqueue(None, &src, &out, "never", ExportPreset::default(), Some(4000), true).unwrap();
        m.cancel(&b).unwrap();
        assert_eq!(wait_done(&rx, &b).status, "cancelled");
        assert_eq!(wait_done(&rx, &a).status, "completed");
    }
}
