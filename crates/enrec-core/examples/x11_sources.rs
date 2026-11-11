//! Prints what the X11 backend sees and the FFmpeg input for the whole desktop.
//! `cargo run -p enrec-core --example x11_sources`
fn main() {
    #[cfg(target_os = "linux")]
    {
        use enrec_core::capture::{x11::X11Backend, CaptureBackend, CaptureRequest};
        let b = X11Backend::new().expect("X11");
        let sources = b.list_sources().expect("sources");
        for s in &sources {
            println!("{}", serde_json::to_string(s).unwrap());
        }
        let input = b.input(&CaptureRequest { source: sources[0].clone(), fps: 30, show_cursor: true }).unwrap();
        println!("ffmpeg input: {}", input.args.join(" "));
    }
}
