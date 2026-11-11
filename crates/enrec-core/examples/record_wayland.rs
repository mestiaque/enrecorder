//! Records 4 s through the Wayland backend (portal → PipeWire) with one pause.
//! `cargo run -p enrec-core --features wayland --example record_wayland -- <out-dir>`
fn main() {
    #[cfg(all(target_os = "linux", feature = "wayland"))]
    {
        use enrec_core::capture::{wayland::WaylandBackend, CaptureBackend, CaptureRequest, CaptureSource};
        use enrec_core::database::Db;
        use enrec_core::media::ffmpeg::{check_capabilities, find_tool, Codec};
        use enrec_core::recording::{plan, session, RecordingOptions};
        use std::sync::Arc;
        use std::time::Duration;

        struct Stderr;
        impl log::Log for Stderr {
            fn enabled(&self, _: &log::Metadata) -> bool {
                true
            }
            fn log(&self, r: &log::Record) {
                eprintln!("[{}] {}", r.target(), r.args());
            }
            fn flush(&self) {}
        }
        log::set_logger(&Stderr).map(|()| log::set_max_level(log::LevelFilter::Info)).unwrap();
        struct Print;
        impl session::Observer for Print {
            fn state(&self, s: &session::Snapshot) {
                println!("state {:?} error={:?}", s.state, s.error);
            }
            fn progress(&self, _: &session::Snapshot) {}
        }
        let out = std::path::PathBuf::from(std::env::args().nth(1).expect("out dir"));
        let ff = find_tool("ffmpeg", None, &[]).expect("ffmpeg");
        let probe = find_tool("ffprobe", None, &[]).map(|t| t.path.into());
        let caps = check_capabilities(&ff).unwrap();
        let backend = WaylandBackend::new().expect("wayland");
        println!("sources: {:?}", backend.list_sources().unwrap());
        let options = RecordingOptions { source: CaptureSource::Portal, fps: 30, resolution: "native".into(), format: "mp4".into(), codec: Codec::H264, encoder: "libx264".into(), quality: "balanced".into(), show_cursor: true, countdown: 0, max_minutes: 0, audio: Default::default(), webcam: Default::default() };
        let input = backend.input(&CaptureRequest { source: CaptureSource::Portal, fps: 30, show_cursor: true }).expect("portal");
        println!("granted {}x{}", input.width, input.height);
        let plan = plan(&options, &input, &caps).unwrap();
        let db = Arc::new(Db::open_in_memory().unwrap());
        let mut h = session::start(session::StartParams { db, ffmpeg: ff.path.into(), ffprobe: probe, input, plan, options, output_dir: out, file_stem: "wayland test".into(), min_free_bytes: 0 }, Arc::new(Print)).unwrap();
        while h.snapshot().state != enrec_core::capture::state::RecState::Recording {
            assert!(!h.snapshot().state.is_final(), "{:?}", h.snapshot().error);
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_secs(3));
        h.send(session::Command::Pause).unwrap();
        std::thread::sleep(Duration::from_secs(1));
        h.send(session::Command::Resume).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        h.send(session::Command::Stop).unwrap();
        h.wait(Duration::from_secs(60));
        let s = h.snapshot();
        println!("result {:?} elapsed={}ms frames={} dropped={} file={:?} error={:?}", s.state, s.elapsed_ms, s.frames, s.dropped, s.output_path, s.error);
    }
}
