//! A running recording. One thread per session drives the state machine
//! ([`crate::capture::state`]), starts and stops FFmpeg segments, tracks time, watches the disk
//! and finalises the file. The UI talks to it through [`SessionHandle`] commands and receives
//! [`Snapshot`]s through an [`Observer`].
//!
//! * **Pause** stops the current FFmpeg segment cleanly; **resume** starts the next one. The
//!   segments are joined without re-encoding on stop, so pausing costs nothing in quality.
//! * **Stop** asks FFmpeg to finish (`q` on stdin) so the last segment is complete; a process
//!   that does not exit in time is killed (the MKV written so far stays readable).
//! * The output is verified with FFprobe before the recording counts as completed. On any
//!   failure the segments are kept next to the recordings for recovery and the session is
//!   marked failed — recorded media is never deleted silently.

use super::{unique_path, EncodePlan, RecordingOptions};
use crate::capture::state::{next, Event, RecState};
use crate::capture::frames::{feed, pipe_input_args, FrameSlot, FrameSource};
use crate::capture::CaptureInput;
use crate::database::{new_id, Db, NewRecording, NewSession};
use crate::error::{AppError, Result};
use crate::media::process::{command, tail};
use crate::media::probe;
use parking_lot::Mutex;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(200);
const DISK_CHECK: Duration = Duration::from_secs(5);
const STOP_GRACE: Duration = Duration::from_secs(10);
/// A segment that dies this soon after starting failed to start (bad device, encoder error).
const STARTUP_WINDOW: Duration = Duration::from_millis(2500);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub session_id: String,
    pub state: RecState,
    /// Seconds left while counting down.
    pub countdown_left: u32,
    /// Recorded time, pauses excluded.
    pub elapsed_ms: u64,
    pub segments: u32,
    pub frames: u64,
    /// Frames FFmpeg dropped because capture or encoding fell behind.
    pub dropped: u64,
    pub fps: f64,
    /// Bytes written so far (all segments).
    pub size_bytes: u64,
    pub encoder: String,
    pub width: u32,
    pub height: u32,
    /// "desktop" | "monitor" | "window" | "region"
    pub source: String,
    pub output_path: Option<String>,
    pub recording_id: Option<String>,
    pub error: Option<String>,
    /// Something the user should know that did not fail the recording (e.g. stopped because the
    /// disk is almost full, or the time limit was reached).
    pub notice: Option<String>,
}

pub trait Observer: Send + Sync + 'static {
    /// The state changed (also once per countdown second).
    fn state(&self, s: &Snapshot);
    /// Timer/size/frames changed (every tick while recording; throttle in the UI layer).
    fn progress(&self, s: &Snapshot);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Pause,
    Resume,
    Stop,
    /// Abort before recording started (during the countdown).
    Cancel,
}

pub struct StartParams {
    pub db: Arc<Db>,
    pub ffmpeg: PathBuf,
    pub ffprobe: Option<PathBuf>,
    pub input: CaptureInput,
    pub plan: EncodePlan,
    pub options: RecordingOptions,
    pub output_dir: PathBuf,
    /// File name without extension, e.g. "Recording 2026-11-11 10-02-33".
    pub file_stem: String,
    /// Stop when the disk has less free space than this.
    pub min_free_bytes: u64,
}

pub struct SessionHandle {
    tx: Sender<Command>,
    snap: Arc<Mutex<Snapshot>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl SessionHandle {
    pub fn send(&self, cmd: Command) -> Result<()> {
        self.tx.send(cmd).map_err(|_| AppError::Other("the recording has already ended".into()))
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().clone()
    }

    pub fn is_active(&self) -> bool {
        self.snap.lock().state.is_active()
    }

    /// Waits for the session thread to finish (after Stop), at most `timeout`.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        while self.thread.as_ref().is_some_and(|t| !t.is_finished()) {
            if start.elapsed() > timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        true
    }
}

/// Starts a session thread. Returns once the session exists; recording begins after the
/// countdown.
pub fn start(p: StartParams, observer: Arc<dyn Observer>) -> Result<SessionHandle> {
    if !p.ffmpeg.is_file() {
        return Err(AppError::FfmpegMissing);
    }
    std::fs::create_dir_all(&p.output_dir).map_err(|e| AppError::InvalidPath(format!("{}: {e}", p.output_dir.display())))?;
    let id = new_id();
    let snap = Snapshot {
        session_id: id.clone(),
        state: RecState::Idle,
        countdown_left: 0,
        elapsed_ms: 0,
        segments: 0,
        frames: 0,
        dropped: 0,
        fps: 0.0,
        size_bytes: 0,
        encoder: p.plan.encoder.clone(),
        width: p.plan.width,
        height: p.plan.height,
        source: p.options.source.kind_label().into(),
        output_path: None,
        recording_id: None,
        error: None,
        notice: None,
    };
    p.db.insert_session(&NewSession {
        id: id.clone(),
        kind: "video".into(),
        state: "preparing".into(),
        source: serde_json::to_string(&p.options.source)?,
        settings: serde_json::to_string(&p.options)?,
        output_path: None,
    })?;
    let snap = Arc::new(Mutex::new(snap));
    let (tx, rx) = mpsc::channel();
    let shared = snap.clone();
    let thread = std::thread::Builder::new()
        .name(format!("recording-{}", &id[..8]))
        .spawn(move || Runner { p, obs: observer, snap: shared, rx, seg_dir: PathBuf::new(), segments: Vec::new(), acc_ms: 0, acc_frames: 0, acc_dropped: 0, source: None, slot: None }.run())
        .map_err(|e| AppError::Other(format!("could not start the recording thread: {e}")))?;
    Ok(SessionHandle { tx, snap, thread: Some(thread) })
}

/// One FFmpeg process writing one segment.
struct Segment {
    child: Child,
    stdin: Option<ChildStdin>,
    path: PathBuf,
    started: Instant,
    frames: Arc<AtomicU64>,
    dropped: Arc<AtomicU64>,
    fps_milli: Arc<AtomicU64>,
    stderr: Arc<Mutex<Vec<String>>>,
    /// Frame-stream input: the thread writing frames to FFmpeg and its stop flag.
    feeder: Option<(std::thread::JoinHandle<()>, Arc<AtomicBool>)>,
}

impl Segment {
    fn stderr_tail(&self) -> String {
        tail(&self.stderr.lock().join("\n"), 3)
    }
}

struct Runner {
    p: StartParams,
    obs: Arc<dyn Observer>,
    snap: Arc<Mutex<Snapshot>>,
    rx: Receiver<Command>,
    seg_dir: PathBuf,
    segments: Vec<PathBuf>,
    /// Recorded time of finished segments.
    acc_ms: u64,
    /// Frames and dropped frames of finished segments.
    acc_frames: u64,
    acc_dropped: u64,
    /// Frame-stream backends (Wayland, Windows): the capture and its latest-frame slot.
    source: Option<Box<dyn FrameSource>>,
    slot: Option<Arc<FrameSlot>>,
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Ends the screen share (portal session / capture item) whatever way the session ended.
        if let Some(src) = self.source.as_mut() {
            src.stop();
        }
    }
}

impl Runner {
    fn set_state(&mut self, ev: Event) -> bool {
        let mut s = self.snap.lock();
        let Some(n) = next(s.state, ev) else {
            log::warn!("recording: {:?} not allowed in {:?}", ev, s.state);
            return false;
        };
        s.state = n;
        let snap = s.clone();
        drop(s);
        if let Err(e) = self.p.db.set_session_state(&snap.session_id, n.as_str(), snap.error.as_deref()) {
            log::error!("recording: could not store state: {e}");
        }
        log::info!("recording {}: {}", &snap.session_id[..8], n.as_str());
        self.obs.state(&snap);
        true
    }

    fn fail(&mut self, msg: String) {
        log::error!("recording failed: {msg}");
        self.snap.lock().error = Some(msg);
        self.set_state(Event::Fail);
        // Nothing usable was recorded: do not leave an empty parts folder behind.
        if self.segments.is_empty() && !self.seg_dir.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.seg_dir);
        }
    }

    fn run(mut self) {
        self.source = self.p.input.frames.take();
        self.set_state(Event::Prepare);
        let id = self.snap.lock().session_id.clone();
        // Segments live in a hidden folder next to the recordings until the file is final.
        self.seg_dir = self.p.output_dir.join(format!(".enrecorder-{id}"));
        if let Err(e) = std::fs::create_dir_all(&self.seg_dir) {
            self.fail(format!("cannot write to {}: {e}", self.p.output_dir.display()));
            return;
        }
        // Frame streams start while preparing: the countdown begins only once pictures arrive,
        // so "recording" always means frames are being written.
        if let Some(src) = self.source.as_mut() {
            let slot = FrameSlot::new(self.p.input.width, self.p.input.height);
            if let Err(e) = src.start(slot.clone()) {
                self.fail(format!("the screen share could not start: {e}"));
                return;
            }
            if !slot.wait_first(Duration::from_secs(10)) {
                self.fail("no picture arrived from the screen share".into());
                return;
            }
            self.slot = Some(slot);
        }
        let countdown = self.p.options.countdown.min(10);
        self.set_state(Event::Ready { countdown: countdown > 0 });
        if countdown > 0 {
            for left in (1..=countdown).rev() {
                self.snap.lock().countdown_left = left;
                let snap = self.snap.lock().clone();
                self.obs.state(&snap);
                let until = Instant::now() + Duration::from_secs(1);
                while Instant::now() < until {
                    match self.rx.recv_timeout(Duration::from_millis(50)) {
                        Ok(Command::Cancel | Command::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            self.set_state(Event::Cancel);
                            let _ = std::fs::remove_dir_all(&self.seg_dir);
                            return;
                        }
                        _ => {}
                    }
                }
            }
            self.snap.lock().countdown_left = 0;
            self.set_state(Event::CountdownDone);
        }
        let mut seg = match self.start_segment() {
            Ok(s) => Some(s),
            Err(e) => {
                self.fail(e.to_string());
                return;
            }
        };
        let mut last_disk = Instant::now();
        let max_ms = u64::from(self.p.options.max_minutes) * 60_000;
        loop {
            let cmd = match self.rx.recv_timeout(TICK) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                // The app went away: finish the file instead of leaving it half-written.
                Err(RecvTimeoutError::Disconnected) => Some(Command::Stop),
            };
            let state = self.snap.lock().state;
            match (cmd, state) {
                (Some(Command::Pause), RecState::Recording) => {
                    if let Some(s) = seg.take() {
                        if let Err(e) = self.finish_segment(s) {
                            self.fail(e.to_string());
                            return;
                        }
                    }
                    self.set_state(Event::Pause);
                }
                (Some(Command::Resume), RecState::Paused) => match self.start_segment() {
                    Ok(s) => {
                        seg = Some(s);
                        self.set_state(Event::Resume);
                    }
                    Err(e) => {
                        // Keep what was recorded before the pause.
                        self.fail(format!("could not resume: {e}"));
                        return;
                    }
                },
                (Some(Command::Stop), RecState::Recording | RecState::Paused) => break,
                _ => {}
            }
            // An FFmpeg that exits by itself has failed (device gone, encoder error, disk full).
            if let Some(s) = seg.as_mut() {
                if let Ok(Some(status)) = s.child.try_wait() {
                    let early = s.started.elapsed() < STARTUP_WINDOW;
                    let detail = s.stderr_tail();
                    let msg = if early { format!("FFmpeg could not start recording: {detail}") } else { format!("FFmpeg stopped unexpectedly ({status}): {detail}") };
                    let s = seg.take().expect("segment present");
                    // A segment that ran for a while is still usable for recovery.
                    if !early && file_size(&s.path) > 0 {
                        self.segments.push(s.path.clone());
                    }
                    self.fail(msg);
                    return;
                }
            }
            self.update_progress(seg.as_ref());
            let elapsed = self.snap.lock().elapsed_ms;
            if max_ms > 0 && elapsed >= max_ms {
                self.snap.lock().notice = Some(format!("Stopped automatically after {} min", self.p.options.max_minutes));
                break;
            }
            if last_disk.elapsed() >= DISK_CHECK {
                last_disk = Instant::now();
                if let Ok(free) = fs4::available_space(&self.p.output_dir) {
                    if free < self.p.min_free_bytes {
                        self.snap.lock().notice = Some("Stopped because the disk is almost full".into());
                        log::warn!("recording: only {free} bytes free, stopping");
                        break;
                    }
                }
            }
        }
        if let Some(s) = seg.take() {
            if let Err(e) = self.finish_segment(s) {
                self.fail(e.to_string());
                return;
            }
        }
        self.update_progress(None);
        self.set_state(Event::Stop);
        match self.finalize() {
            Ok((path, rec_id)) => {
                {
                    let mut s = self.snap.lock();
                    s.output_path = Some(path.to_string_lossy().into_owned());
                    s.recording_id = Some(rec_id);
                }
                if let Err(e) = self.p.db.set_session_output(&id, &path.to_string_lossy()) {
                    log::warn!("recording: could not store the output path: {e}");
                }
                self.set_state(Event::Finalized);
            }
            Err(e) => self.fail(format!("{e}. The recorded parts are kept in {}", self.seg_dir.display())),
        }
    }

    fn start_segment(&mut self) -> Result<Segment> {
        let n = self.segments.len() + 1;
        let path = self.seg_dir.join(format!("part-{n:03}.mkv"));
        let mut args: Vec<String> = vec!["-hide_banner".into(), "-loglevel".into(), "error".into(), "-nostats".into(), "-progress".into(), "pipe:1".into()];
        args.extend(self.p.plan.pre_input.iter().cloned());
        match &self.slot {
            Some(slot) => args.extend(pipe_input_args(slot.width, slot.height, self.p.options.fps)),
            None => args.extend(self.p.input.args.iter().cloned()),
        }
        args.extend(self.p.plan.encode.iter().cloned());
        args.extend(["-f".into(), "matroska".into(), "-y".into(), path.to_string_lossy().into_owned()]);
        log::info!("recording segment {n}: ffmpeg {}", args.join(" "));
        let mut child = command(&self.p.ffmpeg)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Ffmpeg(format!("could not start FFmpeg: {e}")))?;
        let frames = Arc::new(AtomicU64::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let fps_milli = Arc::new(AtomicU64::new(0));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        // -progress key=value lines on stdout.
        if let Some(out) = child.stdout.take() {
            let (f, d, r) = (frames.clone(), dropped.clone(), fps_milli.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(std::result::Result::ok) {
                    let Some((k, v)) = line.split_once('=') else { continue };
                    let v = v.trim();
                    match k {
                        "frame" => f.store(v.parse().unwrap_or(0), Ordering::Relaxed),
                        "drop_frames" => d.store(v.parse().unwrap_or(0), Ordering::Relaxed),
                        "fps" => r.store((v.parse::<f64>().unwrap_or(0.0) * 1000.0) as u64, Ordering::Relaxed),
                        _ => {}
                    }
                }
            });
        }
        if let Some(mut err) = child.stderr.take() {
            let buf = stderr.clone();
            std::thread::spawn(move || {
                let mut text = String::new();
                let mut chunk = [0u8; 4096];
                while let Ok(n) = err.read(&mut chunk) {
                    if n == 0 {
                        break;
                    }
                    text.push_str(&String::from_utf8_lossy(&chunk[..n]));
                    let mut b = buf.lock();
                    while let Some(i) = text.find('\n') {
                        let line: String = text.drain(..=i).collect();
                        log::warn!(target: "ffmpeg", "{}", line.trim_end());
                        b.push(line.trim_end().to_string());
                        if b.len() > 30 {
                            b.remove(0);
                        }
                    }
                }
            });
        }
        let mut stdin = child.stdin.take();
        // Frame streams: a feeder writes the latest frame at the recording rate (see frames.rs).
        let feeder = match (&self.slot, stdin.take()) {
            (Some(slot), Some(mut pipe)) => {
                let (slot, stop, d, fps) = (slot.clone(), Arc::new(AtomicBool::new(false)), dropped.clone(), self.p.options.fps);
                let stop2 = stop.clone();
                let h = std::thread::Builder::new()
                    .name("frame-feeder".into())
                    .spawn(move || feed(&slot, &mut pipe, fps, &stop2, &d))
                    .map_err(|e| AppError::Other(format!("could not start the frame writer: {e}")))?;
                Some((h, stop))
            }
            (_, s) => {
                stdin = s;
                None
            }
        };
        self.snap.lock().segments = n as u32;
        Ok(Segment { child, stdin, path, started: Instant::now(), frames, dropped, fps_milli, stderr, feeder })
    }

    /// Asks FFmpeg to finish the segment and waits; kills it after the grace period.
    fn finish_segment(&mut self, mut s: Segment) -> Result<()> {
        let ran = s.started.elapsed();
        if let Some((h, stop)) = s.feeder.take() {
            // Closing the pipe (the feeder drops it) makes FFmpeg finish the file.
            stop.store(true, Ordering::Release);
            let _ = h.join();
        } else if let Some(mut stdin) = s.stdin.take() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }
        let deadline = Instant::now() + STOP_GRACE;
        let status = loop {
            match s.child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
                _ => {
                    log::warn!("FFmpeg did not stop in time; killing it");
                    let _ = s.child.kill();
                    let _ = s.child.wait();
                    break None;
                }
            }
        };
        if status.is_some_and(|st| !st.success()) {
            log::warn!("FFmpeg ended with {status:?}: {}", s.stderr_tail());
        }
        // A part paused or stopped before its first frame holds no video (and may not even be a
        // valid file); joining it would fail. Leave it out — it contains nothing.
        if !self.segment_has_video(&s) {
            log::info!("dropping part {} after {} ms: it holds no frames", s.path.display(), ran.as_millis());
            let _ = std::fs::remove_file(&s.path);
            return Ok(());
        }
        self.acc_ms += ran.as_millis() as u64;
        self.acc_frames += s.frames.load(Ordering::Relaxed);
        self.acc_dropped += s.dropped.load(Ordering::Relaxed);
        self.segments.push(s.path.clone());
        Ok(())
    }

    /// FFprobe can read a video stream with a duration from the part (or, without FFprobe, FFmpeg
    /// reported frames and the file has more than a header).
    fn segment_has_video(&self, s: &Segment) -> bool {
        if file_size(&s.path) == 0 {
            return false;
        }
        match &self.p.ffprobe {
            Some(fp) => probe::probe(fp, &s.path).is_ok_and(|p| p.video_streams > 0 && p.duration_ms.unwrap_or(0) > 0),
            None => s.frames.load(Ordering::Relaxed) > 0 && file_size(&s.path) > 4096,
        }
    }

    fn update_progress(&mut self, seg: Option<&Segment>) {
        let size: u64 = self.segments.iter().map(|p| file_size(p)).sum::<u64>() + seg.map_or(0, |s| file_size(&s.path));
        let snap = {
            let mut s = self.snap.lock();
            s.elapsed_ms = self.acc_ms + seg.map_or(0, |g| g.started.elapsed().as_millis() as u64);
            s.size_bytes = size;
            s.frames = self.acc_frames + seg.map_or(0, |g| g.frames.load(Ordering::Relaxed));
            s.dropped = self.acc_dropped + seg.map_or(0, |g| g.dropped.load(Ordering::Relaxed));
            if let Some(g) = seg {
                s.fps = g.fps_milli.load(Ordering::Relaxed) as f64 / 1000.0;
            }
            s.clone()
        };
        self.obs.progress(&snap);
    }

    /// Joins the segments into the final file, verifies it and adds it to the library.
    fn finalize(&mut self) -> Result<(PathBuf, String)> {
        if self.segments.is_empty() {
            return Err(AppError::Ffmpeg("nothing was recorded (stopped before the first frame)".into()));
        }
        let final_path = unique_path(&self.p.output_dir, &self.p.file_stem, self.p.plan.extension);
        let tmp = final_path.with_extension(format!("part.{}", self.p.plan.extension));
        if self.segments.len() == 1 && self.p.plan.extension == "mkv" {
            std::fs::rename(&self.segments[0], &tmp)?;
        } else {
            let list = self.seg_dir.join("parts.txt");
            let body: String = self.segments.iter().map(|p| format!("file '{}'\n", p.to_string_lossy().replace('\'', "'\\''"))).collect();
            std::fs::write(&list, body)?;
            let fmt = match self.p.plan.extension {
                "webm" => "webm",
                "mkv" => "matroska",
                _ => "mp4",
            };
            let mut args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin", "-f", "concat", "-safe", "0", "-i"].iter().map(|s| s.to_string()).collect();
            args.push(list.to_string_lossy().into_owned());
            args.extend(["-c".into(), "copy".into()]);
            if fmt == "mp4" {
                args.extend(["-movflags".into(), "+faststart".into()]);
            }
            args.extend(["-f".into(), fmt.into(), "-y".into(), tmp.to_string_lossy().into_owned()]);
            let out = crate::media::process::run(&self.p.ffmpeg, &args, Duration::from_secs(600))?;
            if !out.success {
                let _ = std::fs::remove_file(&tmp);
                return Err(AppError::Ffmpeg(format!("joining the parts failed: {}", tail(&out.stderr, 3))));
            }
        }
        // Verify before calling it done.
        let probe = match &self.p.ffprobe {
            Some(fp) => {
                let pr = probe::probe(fp, &tmp)?;
                if pr.video_streams == 0 || pr.duration_ms.unwrap_or(0) <= 0 {
                    return Err(AppError::Ffmpeg("the finished file has no readable video".into()));
                }
                Some(pr)
            }
            None => None,
        };
        let size = file_size(&tmp);
        if size == 0 {
            return Err(AppError::Ffmpeg("the finished file is empty".into()));
        }
        std::fs::rename(&tmp, &final_path)?;
        let title = final_path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let session_id = self.snap.lock().session_id.clone();
        let rec_id = self.p.db.insert_recording(
            &NewRecording {
                kind: "video".into(),
                title,
                path: final_path.to_string_lossy().into_owned(),
                container: Some(self.p.plan.extension.into()),
                duration_ms: probe.as_ref().and_then(|p| p.duration_ms).or(Some(self.acc_ms as i64)),
                size_bytes: size as i64,
                width: probe.as_ref().and_then(|p| p.width).or(Some(self.p.plan.width as i64)),
                height: probe.as_ref().and_then(|p| p.height).or(Some(self.p.plan.height as i64)),
                session_id: Some(session_id),
            },
            probe.as_ref(),
        )?;
        let _ = std::fs::remove_dir_all(&self.seg_dir);
        Ok((final_path, rec_id))
    }
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureSource;
    use crate::media::ffmpeg::{find_tool, Codec};

    #[derive(Default)]
    struct Record(Mutex<Vec<RecState>>);
    impl Observer for Record {
        fn state(&self, s: &Snapshot) {
            let mut v = self.0.lock();
            if v.last() != Some(&s.state) {
                v.push(s.state);
            }
        }
        fn progress(&self, _: &Snapshot) {}
    }

    fn params(dir: &Path, db: Arc<Db>, ext: &'static str, countdown: u32) -> Option<StartParams> {
        let ff = find_tool("ffmpeg", None, &[])?;
        let probe = find_tool("ffprobe", None, &[]).map(|t| PathBuf::from(t.path));
        // A synthetic source stands in for the screen: same pipeline, no display needed.
        let input = CaptureInput { args: vec!["-re".into(), "-f".into(), "lavfi".into(), "-i".into(), "testsrc2=size=320x240:rate=30".into()], width: 320, height: 240, frames: None };
        let plan = EncodePlan {
            encoder: "libx264".into(),
            pre_input: vec![],
            encode: ["-vf", "format=yuv420p", "-c:v", "libx264", "-preset", "ultrafast", "-g", "30", "-an"].iter().map(|s| s.to_string()).collect(),
            extension: ext,
            width: 320,
            height: 240,
        };
        let options = RecordingOptions {
            source: CaptureSource::Desktop { width: 320, height: 240 },
            fps: 30,
            resolution: "native".into(),
            format: ext.into(),
            codec: Codec::H264,
            encoder: "libx264".into(),
            quality: "balanced".into(),
            show_cursor: true,
            countdown,
            max_minutes: 0,
        };
        Some(StartParams { db, ffmpeg: PathBuf::from(ff.path), ffprobe: probe, input, plan, options, output_dir: dir.to_path_buf(), file_stem: "Test recording".into(), min_free_bytes: 0 })
    }

    fn wait_state(h: &SessionHandle, st: RecState) {
        let t = Instant::now();
        while h.snapshot().state != st {
            assert!(t.elapsed() < Duration::from_secs(20), "timed out waiting for {st:?}, now {:?} ({:?})", h.snapshot().state, h.snapshot().error);
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Real FFmpeg: record, pause, resume, stop → one verified MP4 in the library, parts removed.
    #[test]
    fn records_pauses_and_joins_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let Some(p) = params(dir.path(), db.clone(), "mp4", 1) else { return };
        let obs = Arc::new(Record::default());
        let mut h = start(p, obs.clone()).unwrap();
        wait_state(&h, RecState::Recording);
        std::thread::sleep(Duration::from_millis(1500));
        h.send(Command::Pause).unwrap();
        wait_state(&h, RecState::Paused);
        let paused_at = h.snapshot().elapsed_ms;
        std::thread::sleep(Duration::from_millis(800));
        assert_eq!(h.snapshot().elapsed_ms, paused_at, "the timer stops while paused");
        h.send(Command::Resume).unwrap();
        wait_state(&h, RecState::Recording);
        std::thread::sleep(Duration::from_millis(1500));
        h.send(Command::Stop).unwrap();
        assert!(h.wait(Duration::from_secs(30)));
        let s = h.snapshot();
        assert_eq!(s.state, RecState::Completed, "{:?}", s.error);
        assert_eq!(s.segments, 2);
        assert_eq!(
            *obs.0.lock(),
            vec![RecState::Preparing, RecState::Countdown, RecState::Recording, RecState::Paused, RecState::Recording, RecState::Finalizing, RecState::Completed]
        );
        let out = PathBuf::from(s.output_path.unwrap());
        assert_eq!(out.file_name().unwrap(), "Test recording.mp4");
        let ff = find_tool("ffprobe", None, &[]).unwrap();
        let pr = probe::probe(Path::new(&ff.path), &out).unwrap();
        // Two parts of ~1.5 s each; the pause is not in the file.
        let d = pr.duration_ms.unwrap();
        assert!((2000..4500).contains(&d), "duration {d}");
        assert_eq!(pr.video_codec.as_deref(), Some("h264"));
        // Parts folder removed; library row and session state written.
        assert!(std::fs::read_dir(dir.path()).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".enrecorder-")));
        assert_eq!(db.library_stats().unwrap().videos, 1);
        assert_eq!(db.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'completed'").unwrap(), Some(1));
    }

    /// A frame source like Wayland/Windows: frames from a thread, at an uneven rate.
    struct Synthetic {
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl FrameSource for Synthetic {
        fn start(&mut self, slot: Arc<FrameSlot>) -> Result<()> {
            let stop = self.stop.clone();
            self.thread = Some(std::thread::spawn(move || {
                let mut n = 0u8;
                while !stop.load(Ordering::Acquire) {
                    let frame = vec![n; 320 * 240 * 4];
                    slot.push(&frame, 320, 240, 320 * 4, crate::capture::frames::PixelOrder::Bgrx, 0, 0);
                    n = n.wrapping_add(7);
                    std::thread::sleep(Duration::from_millis(if n.is_multiple_of(3) { 80 } else { 15 }));
                }
            }));
            Ok(())
        }
        fn stop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    /// Frame-stream path with the real FFmpeg: raw frames through the pipe, a pause, stop.
    #[test]
    fn records_from_a_frame_stream() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let Some(mut p) = params(dir.path(), db.clone(), "mkv", 0) else { return };
        let stop = Arc::new(AtomicBool::new(false));
        p.input = CaptureInput { args: vec![], width: 320, height: 240, frames: Some(Box::new(Synthetic { stop: stop.clone(), thread: None })) };
        let mut h = start(p, Arc::new(Record::default())).unwrap();
        wait_state(&h, RecState::Recording);
        std::thread::sleep(Duration::from_millis(1200));
        h.send(Command::Pause).unwrap();
        wait_state(&h, RecState::Paused);
        h.send(Command::Resume).unwrap();
        wait_state(&h, RecState::Recording);
        std::thread::sleep(Duration::from_millis(1200));
        h.send(Command::Stop).unwrap();
        assert!(h.wait(Duration::from_secs(30)));
        let s = h.snapshot();
        assert_eq!(s.state, RecState::Completed, "{:?}", s.error);
        assert!(stop.load(Ordering::Acquire), "the capture is stopped when the session ends");
        let ff = find_tool("ffprobe", None, &[]).unwrap();
        let pr = probe::probe(Path::new(&ff.path), Path::new(&s.output_path.unwrap())).unwrap();
        let d = pr.duration_ms.unwrap();
        assert!((1800..3500).contains(&d), "duration {d}");
        assert_eq!((pr.width, pr.height), (Some(320), Some(240)));
        assert!(s.frames > 40, "frames {}", s.frames);
    }

    /// Pausing right after the start leaves an empty first part; it is dropped, not joined.
    #[test]
    fn an_instant_pause_does_not_break_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let Some(p) = params(dir.path(), db.clone(), "mp4", 0) else { return };
        let mut h = start(p, Arc::new(Record::default())).unwrap();
        wait_state(&h, RecState::Recording);
        h.send(Command::Pause).unwrap();
        wait_state(&h, RecState::Paused);
        h.send(Command::Resume).unwrap();
        wait_state(&h, RecState::Recording);
        std::thread::sleep(Duration::from_millis(1500));
        h.send(Command::Stop).unwrap();
        assert!(h.wait(Duration::from_secs(30)));
        let s = h.snapshot();
        assert_eq!(s.state, RecState::Completed, "{:?}", s.error);
        assert!(s.elapsed_ms < 2500, "the empty part does not count: {}", s.elapsed_ms);
    }

    #[test]
    fn cancel_during_countdown_records_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let Some(p) = params(dir.path(), db.clone(), "mkv", 3) else { return };
        let mut h = start(p, Arc::new(Record::default())).unwrap();
        wait_state(&h, RecState::Countdown);
        h.send(Command::Cancel).unwrap();
        assert!(h.wait(Duration::from_secs(5)));
        assert_eq!(h.snapshot().state, RecState::Cancelled);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert_eq!(db.library_stats().unwrap().videos, 0);
    }

    #[test]
    fn a_broken_capture_fails_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let Some(mut p) = params(dir.path(), db.clone(), "mkv", 0) else { return };
        p.input.args = vec!["-f".into(), "lavfi".into(), "-i".into(), "no_such_filter".into()];
        let mut h = start(p, Arc::new(Record::default())).unwrap();
        assert!(h.wait(Duration::from_secs(15)));
        let s = h.snapshot();
        assert_eq!(s.state, RecState::Failed);
        assert!(s.error.unwrap().contains("could not start"));
        assert_eq!(db.query_i64("SELECT COUNT(*) FROM capture_sessions WHERE state = 'failed' AND error IS NOT NULL").unwrap(), Some(1));
    }
}
