//! End-to-end audio check against a real PulseAudio/PipeWire server: lists devices, then
//! records microphone + system audio (mixed) to MP3 and a screen-less MKV with two tracks.
//! Run: cargo run -p enrec-core --example record_audio -- <out-dir>
use enrec_core::audio::{list_devices, AudioKind, AudioOptions};
use enrec_core::capture::state::RecState;
use enrec_core::capture::CaptureInput;
use enrec_core::database::Db;
use enrec_core::media::ffmpeg::{find_tool, Codec};
use enrec_core::recording::session::{self, Command, Observer, Snapshot, StartParams};
use enrec_core::recording::{plan_audio, AudioRecordingOptions, RecordingOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

struct Log;
impl log::Log for Log {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            println!("[{}] {}", r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

struct Quiet;
impl Observer for Quiet {
    fn state(&self, s: &Snapshot) {
        println!("state {:?}", s.state);
    }
    fn progress(&self, _: &Snapshot) {}
}

fn main() {
    let _ = log::set_logger(&Log).map(|()| log::set_max_level(log::LevelFilter::Info));
    let out = PathBuf::from(std::env::args().nth(1).expect("out dir"));
    let ff = find_tool("ffmpeg", None, &[]).expect("ffmpeg");
    let fp = find_tool("ffprobe", None, &[]).map(|t| PathBuf::from(t.path));
    let devices = list_devices(Some(std::path::Path::new(&ff.path))).expect("devices");
    for d in &devices {
        println!("device {:?} default={} {} ({})", d.kind, d.default, d.name, d.id);
    }
    let mic = devices.iter().find(|d| d.kind == AudioKind::Microphone && d.default).or(devices.iter().find(|d| d.kind == AudioKind::Microphone)).map(|d| d.id.clone());
    let sys = devices.iter().find(|d| d.kind == AudioKind::System && d.default).map(|d| d.id.clone());
    for (format, separate) in [("mp3", false), ("m4a", true)] {
        let opts = AudioRecordingOptions { audio: AudioOptions { microphone: mic.clone(), system: sys.clone(), separate_tracks: separate, mic_volume: 100, system_volume: 100 }, format: format.into(), quality: "balanced".into(), countdown: 0, max_minutes: 0 };
        let plan = plan_audio(&opts).expect("plan");
        let rec = RecordingOptions { source: enrec_core::capture::CaptureSource::Desktop { width: 0, height: 0 }, fps: 30, resolution: "native".into(), format: format.into(), codec: Codec::H264, encoder: plan.encoder.clone(), quality: "balanced".into(), show_cursor: false, countdown: 0, max_minutes: 0, audio: opts.audio.clone() };
        let db = Arc::new(Db::open_in_memory().unwrap());
        let mut h = session::start(StartParams { db, ffmpeg: PathBuf::from(&ff.path), ffprobe: fp.clone(), input: CaptureInput { args: vec![], width: 0, height: 0, frames: None }, plan, options: rec, output_dir: out.clone(), file_stem: format!("audio-{format}"), min_free_bytes: 0 }, Arc::new(Quiet)).expect("start");
        while h.snapshot().state != RecState::Recording {
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(1500));
        println!("level while recording: {:?}", h.snapshot().audio_level);
        h.send(Command::Pause).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        h.send(Command::Resume).unwrap();
        std::thread::sleep(Duration::from_millis(1500));
        h.send(Command::Stop).unwrap();
        h.wait(Duration::from_secs(30));
        let s = h.snapshot();
        println!("{format}: {:?} {:?} tracks={} error={:?}", s.state, s.output_path, s.audio_tracks, s.error);
    }
}
