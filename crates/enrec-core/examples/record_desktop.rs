//! Records the desktop for a few seconds with the real pipeline (X11 backend, verified
//! encoder, session with one pause) and prints the verified result.
//! `cargo run -p enrec-core --example record_desktop -- <out-dir> [encoder]`
fn main() {
    #[cfg(target_os = "linux")]
    {
        use enrec_core::capture::{x11::X11Backend, CaptureBackend, CaptureRequest};
        use enrec_core::database::Db;
        use enrec_core::media::ffmpeg::{check_capabilities, find_tool, Codec};
        use enrec_core::recording::{plan, session, RecordingOptions};
        use std::sync::Arc;
        use std::time::Duration;

        struct Print;
        impl session::Observer for Print {
            fn state(&self, s: &session::Snapshot) {
                println!("state {:?} countdown={} error={:?}", s.state, s.countdown_left, s.error);
            }
            fn progress(&self, _: &session::Snapshot) {}
        }
        let mut args = std::env::args().skip(1);
        let out = std::path::PathBuf::from(args.next().expect("out dir"));
        let encoder = args.next().unwrap_or_else(|| "auto".into());
        let ff = find_tool("ffmpeg", None, &[]).expect("ffmpeg");
        let probe = find_tool("ffprobe", None, &[]).map(|t| t.path.into());
        let caps = check_capabilities(&ff).unwrap();
        let backend = X11Backend::new().unwrap();
        let source = backend.list_sources().unwrap()[0].clone();
        let options = RecordingOptions { source: source.clone(), fps: 30, resolution: "native".into(), format: "mp4".into(), codec: Codec::H264, encoder, quality: "balanced".into(), show_cursor: true, countdown: 0, max_minutes: 0, audio: Default::default(), webcam: Default::default(), extras: Default::default() };
        let input = backend.input(&CaptureRequest { source, fps: 30, show_cursor: true }).unwrap();
        let plan = plan(&options, &input, &caps).unwrap();
        println!("encoder: {}", plan.encoder);
        let db = Arc::new(Db::open_in_memory().unwrap());
        let mut h = session::start(session::StartParams { db, ffmpeg: ff.path.into(), ffprobe: probe, input, plan, options, output_dir: out, file_stem: "x11 test".into(), min_free_bytes: 0, keep_alive: Vec::new(), tracking: None }, Arc::new(Print)).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        h.send(session::Command::Pause).unwrap();
        std::thread::sleep(Duration::from_secs(1));
        h.send(session::Command::Resume).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        h.send(session::Command::Stop).unwrap();
        h.wait(Duration::from_secs(60));
        let s = h.snapshot();
        println!("result {:?} elapsed={}ms frames={} dropped={} size={} file={:?}", s.state, s.elapsed_ms, s.frames, s.dropped, s.size_bytes, s.output_path);
    }
}
