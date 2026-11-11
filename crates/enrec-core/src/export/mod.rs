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
    /// Remove rumble and steady background noise (phase 6).
    pub denoise: bool,
    /// Loudness normalisation to −16 LUFS (podcast/online level).
    pub normalize: bool,
    /// Crop in source pixels (before scaling); None = whole picture.
    pub crop: Option<CropRect>,
    /// Clockwise rotation: 0, 90, 180 or 270.
    pub rotate: u32,
    /// Playback speed 0.25–4 (1 = unchanged); sound keeps its pitch.
    pub speed: f64,
    /// Fade in / out at the start / end of the output, in ms (picture and sound).
    pub fade_in_ms: u64,
    pub fade_out_ms: u64,
    /// Output frame rate (0 = keep).
    pub fps: u32,
    /// "h264" | "hevc" | "av1" | "vp9" ("" = the format's default).
    pub video_codec: String,
    /// Video bitrate in kbit/s (0 = constant quality from `quality`).
    pub video_kbps: u32,
    /// Audio bitrate in kbit/s (0 = default for the quality).
    pub audio_kbps: u32,
    /// Post-processing from the recording's pointer track (see `track`): a highlight that follows
    /// the pointer with click ripples, and/or key combinations.
    pub click_highlights: bool,
    pub keystrokes: bool,
    /// Set by the app: the ASS overlay made from the track for this job.
    pub overlay_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for ExportPreset {
    fn default() -> Self {
        Self {
            kind: "convert".into(),
            format: "mp4".into(),
            quality: "balanced".into(),
            start_ms: None,
            end_ms: None,
            max_height: 0,
            remove_audio: false,
            volume: 100,
            denoise: false,
            normalize: false,
            crop: None,
            rotate: 0,
            speed: 1.0,
            fade_in_ms: 0,
            fade_out_ms: 0,
            fps: 0,
            video_codec: String::new(),
            video_kbps: 0,
            audio_kbps: 0,
            click_highlights: false,
            keystrokes: false,
            overlay_path: None,
        }
    }
}

/// Escapes a path for a filter option value (`ass=…`).
fn filter_path(p: &str) -> String {
    // Inside single quotes only ':' (Windows drive letters) and quotes need escaping.
    let e = p.replace('\\', "/").replace('\'', "'\\''").replace(':', "\\:");
    format!("'{e}'")
}

/// `atempo` only takes 0.5–2 per instance (FFmpeg 4.4): chain several for other speeds.
pub fn atempo_chain(speed: f64) -> Vec<String> {
    let mut left = speed.clamp(0.25, 4.0);
    let mut v = Vec::new();
    while left > 2.0 + 1e-9 {
        v.push("atempo=2.0".to_string());
        left /= 2.0;
    }
    while left < 0.5 - 1e-9 {
        v.push("atempo=0.5".to_string());
        left /= 0.5;
    }
    if (left - 1.0).abs() > 1e-6 {
        v.push(format!("atempo={left:.4}"));
    }
    v
}

/// About how big the result will be (bitrate mode: exact; quality mode: an estimate from the
/// picture size, frame rate and quality). None for unknown lengths.
pub fn estimate_bytes(p: &ExportPreset, out_ms: Option<u64>, width: u32, height: u32, fps: f64, has_audio: bool) -> Option<u64> {
    let secs = out_ms? as f64 / 1000.0;
    let audio_kbps = if !has_audio || p.remove_audio {
        0.0
    } else if p.audio_kbps > 0 {
        p.audio_kbps as f64
    } else {
        match (p.format.as_str(), p.quality.as_str()) {
            ("wav", _) => 1536.0,
            ("flac", _) => 800.0,
            (_, "high") => 192.0,
            (_, "small") => 96.0,
            _ => 140.0,
        }
    };
    if AUDIO.contains(&p.format.as_str()) {
        return Some((audio_kbps * 1000.0 / 8.0 * secs) as u64);
    }
    let (mut w, mut h) = p.crop.map_or((width, height), |c| (c.width, c.height));
    if p.max_height > 0 && h > p.max_height {
        w = w * p.max_height / h.max(1);
        h = p.max_height;
    }
    let fps = if p.fps > 0 { p.fps as f64 } else if fps > 0.0 { fps } else { 30.0 };
    let video_kbps = if p.video_kbps > 0 {
        p.video_kbps as f64
    } else {
        // Bits per pixel per frame for screen content at these settings (measured ballpark).
        let bpp = match (p.kind.as_str(), p.quality.as_str()) {
            ("compress", "small") => 0.012,
            ("compress", "high") => 0.03,
            ("compress", _) => 0.02,
            (_, "high") => 0.07,
            (_, "small") => 0.025,
            _ => 0.04,
        };
        let gif = if p.format == "gif" { 6.0 } else { 1.0 };
        w as f64 * h as f64 * fps * bpp * gif / 1000.0
    };
    Some(((video_kbps + audio_kbps) * 1000.0 / 8.0 * secs) as u64)
}

const VIDEO: &[&str] = &["mp4", "webm", "mkv", "gif"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "flac", "opus", "ogg"];

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
    let speed = if p.speed > 0.0 { p.speed.clamp(0.25, 4.0) } else { 1.0 };
    let out_ms = out_ms.map(|m| (m as f64 / speed) as u64);
    if !matches!(p.rotate, 0 | 90 | 180 | 270) {
        return Err(AppError::InvalidSetting(format!("rotation {} is not offered", p.rotate)));
    }
    // Picture: overlay (on the original picture, where the track's coordinates fit) → crop →
    // rotate → scale → frame rate → speed → fades.
    let mut vf: Vec<String> = Vec::new();
    if let Some(o) = &p.overlay_path {
        vf.push(format!("ass={}", filter_path(o)));
    }
    if let Some(c) = p.crop {
        if c.width < 16 || c.height < 16 {
            return Err(AppError::InvalidSetting("the crop area is too small".into()));
        }
        vf.push(format!("crop={}:{}:{}:{}", c.width & !1, c.height & !1, c.x, c.y));
    }
    match p.rotate {
        90 => vf.push(s("transpose=1")),
        180 => vf.push(s("hflip,vflip")),
        270 => vf.push(s("transpose=2")),
        _ => {}
    }
    vf.push(if p.max_height > 0 { format!("scale=-2:'min({},ih)'", p.max_height) } else { s("scale=trunc(iw/2)*2:trunc(ih/2)*2") });
    if p.fps > 0 {
        vf.push(format!("fps={}", p.fps.clamp(1, 120)));
    }
    if (speed - 1.0).abs() > 1e-6 {
        vf.push(format!("setpts=PTS/{speed:.4}"));
    }
    let fade_secs = |ms: u64| format!("{:.3}", ms as f64 / 1000.0);
    if p.fade_in_ms > 0 {
        vf.push(format!("fade=t=in:st=0:d={}", fade_secs(p.fade_in_ms)));
    }
    if let (true, Some(total)) = (p.fade_out_ms > 0, out_ms) {
        vf.push(format!("fade=t=out:st={}:d={}", fade_secs(total.saturating_sub(p.fade_out_ms)), fade_secs(p.fade_out_ms)));
    }
    let vol = (p.volume != 100).then(|| format!("volume={:.2}", p.volume.min(400) as f64 / 100.0));
    let mut af: Vec<String> = Vec::new();
    if p.denoise {
        af.push(s("highpass=f=80,afftdn=nr=12:nf=-30:tn=1"));
    }
    if let Some(v) = &vol {
        af.push(v.clone());
    }
    af.extend(atempo_chain(speed));
    if p.normalize {
        af.push(s("loudnorm=I=-16:TP=-1.5:LRA=11"));
    }
    if p.fade_in_ms > 0 {
        af.push(format!("afade=t=in:st=0:d={}", fade_secs(p.fade_in_ms)));
    }
    if let (true, Some(total)) = (p.fade_out_ms > 0, out_ms) {
        af.push(format!("afade=t=out:st={}:d={}", fade_secs(total.saturating_sub(p.fade_out_ms)), fade_secs(p.fade_out_ms)));
    }
    let akbps = |def: String| if p.audio_kbps > 0 { format!("{}k", p.audio_kbps.clamp(32, 512)) } else { def };
    match p.format.as_str() {
        "gif" => {
            let h = if p.max_height > 0 { p.max_height } else { 480 };
            let fps = if p.fps > 0 { p.fps.min(30).to_string() } else { pick("15", "12", "10") };
            // Same picture chain without the final scale, then the palette steps.
            let pre: Vec<&String> = vf.iter().filter(|f| !f.starts_with("scale=") && !f.starts_with("fps=") && !f.starts_with("fade=")).collect();
            let pre = if pre.is_empty() { String::new() } else { format!("{},", pre.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(",")) };
            a.extend([s("-filter_complex"), format!("[0:v]{pre}fps={fps},scale=-2:'min({h},ih)':flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4"), s("-an"), s("-loop"), s("0")]);
        }
        "mp4" | "mkv" | "webm" => {
            let codec = match (p.format.as_str(), p.video_codec.as_str()) {
                ("webm", "av1") => "av1",
                ("webm", _) => "vp9",
                ("mp4", "vp9") => return Err(AppError::InvalidSetting("VP9 cannot be stored in MP4; choose WebM or MKV".into())),
                (_, "hevc") => "hevc",
                (_, "av1") => "av1",
                (_, "vp9") => "vp9",
                _ => "h264",
            };
            a.extend([s("-map"), s("0:v:0"), s("-vf"), vf.join(",")]);
            let compress = p.kind == "compress";
            match codec {
                "hevc" => {
                    a.extend([s("-c:v"), s("libx265"), s("-preset"), s(if compress { "medium" } else { "fast" }), s("-pix_fmt"), s("yuv420p")]);
                    if p.format == "mp4" {
                        a.extend([s("-tag:v"), s("hvc1")]);
                    }
                }
                "av1" => a.extend([s("-c:v"), s("libaom-av1"), s("-cpu-used"), s("6"), s("-row-mt"), s("1"), s("-pix_fmt"), s("yuv420p")]),
                "vp9" => a.extend([s("-c:v"), s("libvpx-vp9"), s("-deadline"), s("good"), s("-cpu-used"), s("4"), s("-row-mt"), s("1"), s("-pix_fmt"), s("yuv420p")]),
                _ => a.extend([s("-c:v"), s("libx264"), s("-preset"), s(if compress { "medium" } else { "veryfast" }), s("-pix_fmt"), s("yuv420p")]),
            }
            if p.video_kbps > 0 {
                let k = p.video_kbps.clamp(100, 200_000);
                a.extend([s("-b:v"), format!("{k}k"), s("-maxrate"), format!("{}k", k * 3 / 2), s("-bufsize"), format!("{}k", k * 2)]);
            } else {
                let crf = match (codec, compress) {
                    ("vp9" | "av1", false) => pick("28", "33", "38"),
                    ("vp9" | "av1", true) => pick("33", "37", "42"),
                    ("hevc", false) => pick("20", "24", "28"),
                    ("hevc", true) => pick("26", "30", "34"),
                    (_, true) => pick("24", "28", "32"),
                    _ => pick("18", "22", "26"),
                };
                if matches!(codec, "vp9" | "av1") {
                    a.extend([s("-b:v"), s("0")]);
                }
                a.extend([s("-crf"), crf]);
            }
            if p.remove_audio {
                a.push(s("-an"));
            } else {
                if p.format == "webm" {
                    a.extend([s("-map"), s("0:a?"), s("-c:a"), s("libopus"), s("-b:a"), akbps(s("128k"))]);
                } else {
                    a.extend([s("-map"), s("0:a?"), s("-c:a"), s("aac"), s("-b:a"), akbps(if compress { s("96k") } else { pick("192k", "160k", "128k") })]);
                }
                if !af.is_empty() {
                    a.extend([s("-af"), af.join(",")]);
                }
            }
            if p.format == "mp4" {
                a.extend([s("-movflags"), s("+faststart")]);
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
                "mp3" => a.extend([s("-c:a"), s("libmp3lame"), s("-b:a"), akbps(pick("256k", "192k", "128k"))]),
                "m4a" => a.extend([s("-c:a"), s("aac"), s("-b:a"), akbps(pick("256k", "160k", "96k"))]),
                "opus" => a.extend([s("-c:a"), s("libopus"), s("-b:a"), akbps(pick("160k", "112k", "64k"))]),
                "ogg" => a.extend([s("-c:a"), s("libvorbis"), s("-q:a"), pick("7", "5", "3")]),
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
        "ogg" => "ogg",
        _ => "mp4",
    }
}

/// Writes the ASS overlay for an export from a pointer track (temporary file) and sets it on the
/// preset. Trim start/end map the track's time to the output; the overlay goes on before speed
/// changes, so speed needs no mapping.
pub fn attach_overlay(preset: &mut ExportPreset, track: &crate::track::Track) -> Result<PathBuf> {
    let ass = crate::track::to_ass(track, track.width, track.height, preset.click_highlights, preset.keystrokes, preset.start_ms.unwrap_or(0), 1.0, preset.end_ms);
    let path = std::env::temp_dir().join(format!("enrecorder-overlay-{}.ass", new_id()));
    std::fs::write(&path, ass)?;
    preset.overlay_path = Some(path.to_string_lossy().into_owned());
    Ok(path)
}

/// Saves one frame of a video as PNG (thumbnail extraction).
pub fn extract_frame(ffmpeg: &Path, src: &Path, at_ms: u64, dst: &Path) -> Result<()> {
    let args = ["-hide_banner", "-loglevel", "error", "-nostdin", "-ss", &secs(at_ms), "-i", &src.to_string_lossy(), "-frames:v", "1", "-y", &dst.to_string_lossy()].iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let out = crate::media::process::run(ffmpeg, &args, Duration::from_secs(30))?;
    if !out.success || std::fs::metadata(dst).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(AppError::Ffmpeg(format!("could not read a frame at {}: {}", secs(at_ms), crate::media::process::tail(&out.stderr, 2))));
    }
    Ok(())
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

#[derive(Clone)]
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
    /// The job being exported (for duplicate detection).
    current: Mutex<Option<Job>>,
    worker: Mutex<bool>,
    observer: Arc<dyn ExportObserver>,
}

impl ExportManager {
    pub fn new(db: Arc<Db>, observer: Arc<dyn ExportObserver>) -> Arc<Self> {
        Arc::new(Self { db, ffmpeg: Mutex::new(None), ffprobe: Mutex::new(None), queue: Mutex::new(VecDeque::new()), running: Mutex::new(None), current: Mutex::new(None), worker: Mutex::new(false), observer })
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
        // The same export of the same file twice in a row is almost always a double click.
        let key = serde_json::to_string(&preset)?;
        let dup = |j: &Job| j.src == src && serde_json::to_string(&j.preset).ok().as_deref() == Some(key.as_str());
        if self.queue.lock().iter().any(dup) || self.current.lock().as_ref().is_some_and(dup) {
            return Err(AppError::Other("this export is already in the queue".into()));
        }
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

    /// Runs a failed or cancelled job again with the same settings (a new job, same source).
    pub fn retry(self: &Arc<Self>, id: &str) -> Result<String> {
        let row: Option<(Option<String>, String, String, String, String)> = self
            .db
            .conn
            .lock()
            .query_row("SELECT source_id, source_path, output_path, preset, status FROM export_jobs WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .ok();
        let (source_id, src, out, preset, status) = row.ok_or_else(|| AppError::NotFound(format!("export {id}")))?;
        if !matches!(status.as_str(), "failed" | "cancelled") {
            return Err(AppError::Other("only failed or cancelled exports can be retried".into()));
        }
        let preset: ExportPreset = serde_json::from_str(&preset)?;
        if preset.overlay_path.as_deref().is_some_and(|o| !Path::new(o).is_file()) {
            return Err(AppError::Other("start this export again from the editor (its overlay was removed)".into()));
        }
        let src = PathBuf::from(src);
        let out = PathBuf::from(out);
        let (dur, kind): (Option<i64>, Option<String>) = self.db.conn.lock().query_row("SELECT duration_ms, kind FROM recordings WHERE path = ?1", params![src.to_string_lossy()], |r| Ok((r.get(0)?, r.get(1)?))).unwrap_or((None, None));
        let has_video = kind.as_deref().is_none_or(|k| k == "video");
        let stem = out.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Export".into());
        let dir = out.parent().map(Path::to_path_buf).unwrap_or_default();
        self.enqueue(source_id.as_deref(), &src, &dir, &stem, preset, dur.map(|d| d.max(0) as u64), has_video)
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
        *self.current.lock() = Some(job.clone());
        let result = self.execute(&job, &stop);
        *self.running.lock() = None;
        *self.current.lock() = None;
        // A finished overlay is not needed again (a retry of a failed job keeps it).
        if let (Ok(()), Some(o)) = (&result, &job.preset.overlay_path) {
            let _ = std::fs::remove_file(o);
        }
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
        let mp3 = build_args(&ExportPreset { kind: "audio".into(), format: "mp3".into(), volume: 150, denoise: true, normalize: true, ..Default::default() }, src, dst, Some(1000), false).unwrap().0;
        let af = &mp3[mp3.iter().position(|x| x == "-af").unwrap() + 1];
        assert!(mp3.contains(&"-vn".to_string()) && af.starts_with("highpass") && af.contains("volume=1.50") && af.ends_with("LRA=11"), "{af}");
    }

    #[test]
    fn editing_arguments() {
        let src = Path::new("/in.mkv");
        let dst = Path::new("/out.mp4");
        let p = ExportPreset {
            crop: Some(CropRect { x: 10, y: 20, width: 641, height: 361 }),
            rotate: 90,
            speed: 2.0,
            fade_in_ms: 500,
            fade_out_ms: 1000,
            fps: 24,
            video_codec: "hevc".into(),
            video_kbps: 2000,
            overlay_path: Some("/tmp/o.ass".into()),
            ..Default::default()
        };
        let (a, ms) = build_args(&p, src, dst, Some(10_000), true).unwrap();
        assert_eq!(ms, Some(5000), "twice as fast → half as long");
        let vf = &a[a.iter().position(|x| x == "-vf").unwrap() + 1];
        assert_eq!(vf, "ass='/tmp/o.ass',crop=640:360:10:20,transpose=1,scale=trunc(iw/2)*2:trunc(ih/2)*2,fps=24,setpts=PTS/2.0000,fade=t=in:st=0:d=0.500,fade=t=out:st=4.000:d=1.000");
        let af = &a[a.iter().position(|x| x == "-af").unwrap() + 1];
        assert!(af.contains("atempo=2.0") && af.contains("afade=t=out:st=4.000"), "{af}");
        assert!(a.windows(2).any(|w| w == ["-c:v", "libx265"]) && a.windows(2).any(|w| w == ["-b:v", "2000k"]) && a.contains(&"hvc1".to_string()));
        assert!(build_args(&ExportPreset { video_codec: "vp9".into(), ..Default::default() }, src, dst, None, true).is_err(), "VP9 in MP4");
        assert!(build_args(&ExportPreset { rotate: 45, ..Default::default() }, src, dst, None, true).is_err());
        assert_eq!(atempo_chain(0.25), vec!["atempo=0.5", "atempo=0.5000"]);
        assert_eq!(atempo_chain(3.0), vec!["atempo=2.0", "atempo=1.5000"]);
        assert!(atempo_chain(1.0).is_empty());
        assert_eq!(filter_path("C:\\Users\\a.ass"), "'C\\:/Users/a.ass'");
        // Estimates: bitrate mode is exact; audio formats use the audio rate only.
        assert_eq!(estimate_bytes(&ExportPreset { video_kbps: 1000, audio_kbps: 128, ..Default::default() }, Some(8000), 1920, 1080, 30.0, true), Some(1_128_000));
        assert_eq!(estimate_bytes(&ExportPreset { format: "mp3".into(), audio_kbps: 128, ..Default::default() }, Some(8000), 0, 0, 0.0, true), Some(128_000));
        assert!(estimate_bytes(&ExportPreset::default(), None, 1920, 1080, 30.0, true).is_none());
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
        let mp3 = m.enqueue(None, &src, &out, "clip", ExportPreset { kind: "audio".into(), format: "mp3".into(), denoise: true, normalize: true, ..Default::default() }, Some(4000), true).unwrap();
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
        // Editing: crop + rotate + speed + fades + a click overlay, then a frame grab.
        let track = crate::track::Track {
            version: 1,
            width: 320,
            height: 240,
            events: vec![
                crate::track::TrackEvent { t: 200, x: 100, y: 100, click: Some("left".into()), key: None },
                crate::track::TrackEvent { t: 900, x: 150, y: 120, click: None, key: Some("Ctrl+C".into()) },
            ],
        };
        let mut edit = ExportPreset { crop: Some(CropRect { x: 0, y: 0, width: 240, height: 160 }), rotate: 90, speed: 2.0, fade_in_ms: 300, fade_out_ms: 300, click_highlights: true, keystrokes: true, ..Default::default() };
        let overlay = attach_overlay(&mut edit, &track).unwrap();
        let e = m.enqueue(None, &src, &out, "edit", edit.clone(), Some(4000), true).unwrap();
        assert!(m.enqueue(None, &src, &out, "edit", edit, Some(4000), true).is_err() || matches!(wait_done(&rx, &e).status.as_str(), "completed"), "a double click does not queue twice");
        let u = wait_done(&rx, &e);
        assert_eq!(u.status, "completed", "{:?}", u.error);
        let pe = probe::probe(Path::new(&fp.path), &out.join("edit.mp4")).unwrap();
        assert_eq!((pe.width, pe.height), (Some(160), Some(240)), "cropped 240×160, then turned");
        let d = pe.duration_ms.unwrap();
        assert!((1800..2300).contains(&d), "4 s at double speed: {d}");
        assert!(!overlay.exists(), "the overlay is removed after a successful export");
        extract_frame(Path::new(&ff.path), &src, 1500, &out.join("frame.png")).unwrap();
        assert!(out.join("frame.png").metadata().unwrap().len() > 1000);
        assert!(extract_frame(Path::new(&ff.path), &src, 60_000, &out.join("none.png")).is_err(), "past the end");
        // Retry a failed job: a source that disappears fails; after it is back, retry works.
        let gone = dir.path().join("gone.mkv");
        std::fs::copy(&src, &gone).unwrap();
        let f = m.enqueue(None, &gone, &out, "retry", ExportPreset::default(), Some(4000), true).unwrap();
        // Remove it right away: the job (or its successor) will fail on the missing input.
        std::fs::remove_file(&gone).unwrap();
        let fu = wait_done(&rx, &f);
        if fu.status == "failed" {
            std::fs::copy(&src, &gone).unwrap();
            let r = m.retry(&f).unwrap();
            assert_eq!(wait_done(&rx, &r).status, "completed");
        }
        assert!(m.retry(&e).is_err(), "a completed job is not retried");
        // Cancel a queued job.
        let a = m.enqueue(None, &src, &out, "slow", ExportPreset { kind: "compress".into(), ..Default::default() }, Some(4000), true).unwrap();
        let b = m.enqueue(None, &src, &out, "never", ExportPreset::default(), Some(4000), true).unwrap();
        m.cancel(&b).unwrap();
        assert_eq!(wait_done(&rx, &b).status, "cancelled");
        assert_eq!(wait_done(&rx, &a).status, "completed");
    }
}
