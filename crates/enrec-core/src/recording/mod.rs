//! Video recording: options from the UI, the FFmpeg encode plan built from them, and the
//! session that runs it ([`session`]).
//!
//! Every recording is written as MKV *segments* (a new segment after each pause). MKV stays
//! readable when the app or the computer dies mid-recording, so nothing is lost; on stop the
//! segments are joined without re-encoding into the chosen container (MP4, MKV or WebM).

pub mod session;

use crate::capture::{CaptureInput, CaptureSource};
use crate::error::{AppError, Result};
use crate::media::ffmpeg::{render_node, Capabilities, Codec, Vendor, CANDIDATES};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingOptions {
    pub source: CaptureSource,
    /// 24, 30, 60 or 120.
    pub fps: u32,
    /// "native" or a maximum height: "720", "1080", "1440", "2160" (never upscaled).
    pub resolution: String,
    /// "mp4" | "mkv" | "webm"
    pub format: String,
    pub codec: Codec,
    /// "auto" (best verified, hardware first) or an encoder name.
    pub encoder: String,
    /// "high" | "balanced" | "small"
    pub quality: String,
    pub show_cursor: bool,
    /// Seconds before recording starts (0–10).
    pub countdown: u32,
    /// Stop automatically after this many minutes (0 = no limit).
    pub max_minutes: u32,
    /// Microphone and/or system audio (phase 3). Off when absent.
    #[serde(default)]
    pub audio: crate::audio::AudioOptions,
    /// Camera picture in a corner (phase 6). Off when absent.
    #[serde(default)]
    pub webcam: crate::webcam::WebcamOptions,
}

/// What a recording needs from FFmpeg besides the capture input.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodePlan {
    pub encoder: String,
    /// Arguments that must come before the input (hardware device setup).
    pub pre_input: Vec<String>,
    /// Filters, encoder and its quality settings (no output file).
    pub encode: Vec<String>,
    /// Extra video inputs (webcam), placed right after the capture input.
    pub video_inputs: Vec<String>,
    /// Audio device inputs, placed after the video inputs.
    pub audio_inputs: Vec<String>,
    /// Audio-only recording (no video input, verified for audio).
    pub audio_only: bool,
    /// Final container extension.
    pub extension: &'static str,
    pub width: u32,
    pub height: u32,
}

pub const FORMATS: &[&str] = &["mp4", "mkv", "webm"];

fn codec_allowed(format: &str, codec: Codec) -> bool {
    match format {
        "mp4" => matches!(codec, Codec::H264 | Codec::Hevc | Codec::Av1),
        "webm" => matches!(codec, Codec::Vp9 | Codec::Av1),
        "mkv" => true,
        _ => false,
    }
}

/// The encoder to use: the requested one if it is verified for this codec, or with "auto"
/// the first verified one in preference order (hardware before software).
pub fn choose_encoder(caps: &Capabilities, codec: Codec, requested: &str) -> Result<String> {
    let verified = |name: &str| caps.encoders.iter().any(|e| e.name == name && e.verified && e.codec == codec);
    if requested != "auto" {
        return if verified(requested) {
            Ok(requested.to_string())
        } else {
            Err(AppError::InvalidSetting(format!("{requested} did not pass its test on this computer; choose another encoder")))
        };
    }
    CANDIDATES
        .iter()
        .filter(|c| c.codec == codec)
        .find(|c| verified(c.name))
        .map(|c| c.name.to_string())
        .ok_or_else(|| AppError::Unsupported(format!("No verified {} encoder on this computer", codec_name(codec))))
}

pub fn codec_name(c: Codec) -> &'static str {
    match c {
        Codec::H264 => "H.264",
        Codec::Hevc => "H.265",
        Codec::Av1 => "AV1",
        Codec::Vp9 => "VP9",
    }
}

/// Output size after the resolution limit (never upscaled, always even).
pub fn output_size(w: u32, h: u32, resolution: &str) -> (u32, u32) {
    let even = |v: u32| (v & !1).max(2);
    match resolution.parse::<u32>() {
        Ok(max_h) if h > max_h => {
            let nw = (w as f64 * max_h as f64 / h as f64).round() as u32;
            (even(nw), even(max_h))
        }
        _ => (even(w), even(h)),
    }
}

fn level(quality: &str, high: u32, balanced: u32, small: u32) -> String {
    match quality {
        "high" => high,
        "small" => small,
        _ => balanced,
    }
    .to_string()
}

/// Builds the encode plan; checks format/codec/fps and picks the encoder.
pub fn plan(opts: &RecordingOptions, input: &CaptureInput, caps: &Capabilities) -> Result<EncodePlan> {
    if !FORMATS.contains(&opts.format.as_str()) {
        return Err(AppError::InvalidSetting(format!("unknown format {}", opts.format)));
    }
    if !codec_allowed(&opts.format, opts.codec) {
        return Err(AppError::InvalidSetting(format!("{} cannot be stored in {}", codec_name(opts.codec), opts.format.to_uppercase())));
    }
    if !matches!(opts.fps, 24 | 30 | 60 | 120) {
        return Err(AppError::InvalidSetting(format!("{} fps is not offered", opts.fps)));
    }
    let encoder = choose_encoder(caps, opts.codec, &opts.encoder)?;
    let vendor = CANDIDATES.iter().find(|c| c.name == encoder).map(|c| c.vendor).unwrap_or(Vendor::Software);
    let (w, h) = output_size(input.width, input.height, &opts.resolution);
    let q = opts.quality.as_str();

    // Size and pixel format; hardware encoders take frames uploaded to the GPU.
    let base = if (w, h) != (input.width & !1, input.height & !1) {
        format!("scale={w}:{h}:flags=bicubic")
    } else {
        "crop=trunc(iw/2)*2:trunc(ih/2)*2".to_string()
    };
    let mut pre_input = Vec::new();
    match (vendor, encoder.as_str()) {
        (Vendor::Vaapi, _) => {
            let dev = render_node().map_or_else(|| "/dev/dri/renderD128".to_string(), |p| p.to_string_lossy().into_owned());
            pre_input.extend(["-vaapi_device".to_string(), dev]);
        }
        (Vendor::Intel, _) => {
            pre_input.extend(["-init_hw_device".into(), "qsv=hw".into(), "-filter_hw_device".into(), "hw".into()]);
        }
        _ => {}
    }
    let after = match vendor {
        Vendor::Vaapi => "format=nv12,hwupload",
        Vendor::Intel => "format=nv12,hwupload=extra_hw_frames=64,format=qsv",
        _ => "format=yuv420p",
    };
    // Webcam: a second video input overlaid before the pixel format / GPU upload.
    let cam = opts.webcam.device.as_deref().filter(|d| !d.is_empty());
    let video_inputs = cam.map(|d| crate::webcam::input_args(d, opts.fps)).unwrap_or_default();
    let mut enc: Vec<String> = match cam {
        Some(_) => vec!["-filter_complex".into(), crate::webcam::overlay_graph(&opts.webcam, 1, w, &base, after), "-map".into(), "[vout]".into()],
        None => vec!["-vf".into(), format!("{base},{after}")],
    };
    enc.extend(["-c:v".into(), encoder.clone(), "-g".into(), (opts.fps * 2).to_string()]);
    let push = |enc: &mut Vec<String>, items: &[&str]| enc.extend(items.iter().map(|s| s.to_string()));
    match encoder.as_str() {
        "libx264" => {
            push(&mut enc, &["-preset", "veryfast", "-tune", "zerolatency"]);
            enc.extend(["-crf".into(), level(q, 20, 24, 28)]);
        }
        "libx265" => {
            push(&mut enc, &["-preset", "ultrafast"]);
            enc.extend(["-crf".into(), level(q, 22, 26, 30)]);
        }
        "libvpx-vp9" => {
            push(&mut enc, &["-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1", "-b:v", "0"]);
            enc.extend(["-crf".into(), level(q, 30, 34, 38)]);
        }
        "libaom-av1" => {
            push(&mut enc, &["-usage", "realtime", "-cpu-used", "8", "-row-mt", "1", "-b:v", "0"]);
            enc.extend(["-crf".into(), level(q, 30, 35, 40)]);
        }
        "libsvtav1" => {
            push(&mut enc, &["-preset", "10"]);
            enc.extend(["-crf".into(), level(q, 30, 35, 40)]);
        }
        e if e.ends_with("_vaapi") => {
            push(&mut enc, &["-rc_mode", "CQP"]);
            enc.extend(["-qp".into(), level(q, 20, 25, 30)]);
        }
        e if e.ends_with("_nvenc") => {
            push(&mut enc, &["-preset", "p4", "-rc", "vbr", "-b:v", "0"]);
            enc.extend(["-cq".into(), level(q, 21, 25, 29)]);
        }
        e if e.ends_with("_qsv") => enc.extend(["-global_quality".into(), level(q, 20, 25, 30)]),
        e if e.ends_with("_amf") => {
            push(&mut enc, &["-rc", "cqp"]);
            enc.extend(["-qp_i".into(), level(q, 20, 25, 30), "-qp_p".into(), level(q, 22, 27, 32)]);
        }
        _ => {
            // Media Foundation and others: a bitrate that suits the size.
            let mbps = ((w as f64 * h as f64 * opts.fps as f64) / 1e6 * match q { "high" => 0.12, "small" => 0.05, _ => 0.08 }).max(1.0);
            enc.extend(["-b:v".into(), format!("{mbps:.0}M")]);
        }
    }
    let extension = match opts.format.as_str() {
        "mkv" => "mkv",
        "webm" => "webm",
        _ => "mp4",
    };
    // Audio from the same FFmpeg process: one clock, one file per segment.
    let first_audio = 1 + usize::from(cam.is_some());
    let audio = crate::audio::plan(&opts.audio, first_audio, crate::audio::codec_for(extension), true);
    if audio.inputs.is_empty() {
        enc.push("-an".into());
    } else {
        if cam.is_none() {
            enc.extend(["-map".into(), "0:v".into()]);
        }
        enc.extend(audio.output);
    }
    Ok(EncodePlan { encoder, pre_input, encode: enc, video_inputs, audio_inputs: audio.inputs, audio_only: false, extension, width: w, height: h })
}

/// Options of the Audio Recorder (no video).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioRecordingOptions {
    pub audio: crate::audio::AudioOptions,
    /// "mp3" | "m4a" | "opus" | "flac" | "wav"
    pub format: String,
    /// "high" | "balanced" | "small"
    pub quality: String,
    pub countdown: u32,
    pub max_minutes: u32,
}

/// Encode plan for an audio-only recording. Segments are Matroska (any codec), joined on stop.
pub fn plan_audio(opts: &AudioRecordingOptions) -> Result<EncodePlan> {
    if opts.audio.is_empty() {
        return Err(AppError::InvalidSetting("Choose a microphone or system audio to record".into()));
    }
    let (codec, ext) = crate::audio::audio_format(&opts.format, &opts.quality)?;
    let codec_refs: Vec<&str> = codec.iter().map(|s| s.as_str()).collect();
    // MP3, WAV, FLAC and Opus files hold one audio stream: two sources are mixed there.
    let mut a = opts.audio.clone();
    if ext != "m4a" {
        a.separate_tracks = false;
    }
    let audio = crate::audio::plan(&a, 0, &codec_refs, true);
    Ok(EncodePlan { encoder: codec[1].clone(), pre_input: vec![], encode: audio.output, video_inputs: vec![], audio_inputs: audio.inputs, audio_only: true, extension: ext, width: 0, height: 0 })
}

/// `dir/stem.ext`, or `dir/stem (2).ext` … when the name is taken.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let clean: String = stem.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let clean = clean.trim().trim_matches('.');
    let clean = if clean.is_empty() { "Recording" } else { clean };
    let first = dir.join(format!("{clean}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{clean} ({n}).{ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::ffmpeg::{EncoderStatus, ToolInfo};

    fn caps(verified: &[&str]) -> Capabilities {
        Capabilities {
            ffmpeg: ToolInfo { path: "/usr/bin/ffmpeg".into(), version: "4.4".into() },
            encoders: CANDIDATES.iter().map(|c| EncoderStatus { name: c.name.into(), codec: c.codec, vendor: c.vendor, listed: true, verified: verified.contains(&c.name), detail: String::new() }).collect(),
            checked_at: 0,
        }
    }

    fn opts() -> RecordingOptions {
        RecordingOptions {
            source: CaptureSource::Desktop { width: 1920, height: 1080 },
            fps: 30,
            resolution: "native".into(),
            format: "mp4".into(),
            codec: Codec::H264,
            encoder: "auto".into(),
            quality: "balanced".into(),
            show_cursor: true,
            countdown: 3,
            max_minutes: 0,
            audio: Default::default(),
            webcam: Default::default(),
        }
    }

    fn input(w: u32, h: u32) -> CaptureInput {
        CaptureInput { args: vec![], width: w, height: h, frames: None }
    }

    #[test]
    fn auto_prefers_verified_hardware() {
        assert_eq!(choose_encoder(&caps(&["libx264", "h264_vaapi"]), Codec::H264, "auto").unwrap(), "h264_vaapi");
        assert_eq!(choose_encoder(&caps(&["libx264"]), Codec::H264, "auto").unwrap(), "libx264");
        assert!(choose_encoder(&caps(&["libx264"]), Codec::Hevc, "auto").is_err());
        // A named encoder must be verified for that codec.
        assert!(choose_encoder(&caps(&["libx264"]), Codec::H264, "h264_nvenc").is_err());
        assert!(choose_encoder(&caps(&["libx265"]), Codec::H264, "libx265").is_err());
    }

    #[test]
    fn format_codec_rules() {
        let c = caps(&["libx264", "libvpx-vp9"]);
        assert!(plan(&RecordingOptions { format: "webm".into(), ..opts() }, &input(1920, 1080), &c).is_err());
        assert!(plan(&RecordingOptions { format: "webm".into(), codec: Codec::Vp9, ..opts() }, &input(1920, 1080), &c).is_ok());
        assert!(plan(&RecordingOptions { format: "mp4".into(), codec: Codec::Vp9, ..opts() }, &input(1920, 1080), &c).is_err());
        assert!(plan(&RecordingOptions { format: "avi".into(), ..opts() }, &input(1920, 1080), &c).is_err());
        assert!(plan(&RecordingOptions { fps: 25, ..opts() }, &input(1920, 1080), &c).is_err());
    }

    #[test]
    fn sizes_are_limited_and_even() {
        assert_eq!(output_size(1920, 1080, "native"), (1920, 1080));
        assert_eq!(output_size(1920, 1080, "720"), (1280, 720));
        assert_eq!(output_size(1280, 720, "1080"), (1280, 720), "never upscaled");
        assert_eq!(output_size(1001, 701, "native"), (1000, 700));
        assert_eq!(output_size(2560, 1600, "1080"), (1728, 1080));
    }

    #[test]
    fn software_plan() {
        let p = plan(&opts(), &input(1920, 1080), &caps(&["libx264"])).unwrap();
        assert_eq!(p.encoder, "libx264");
        assert!(p.pre_input.is_empty());
        assert!(p.encode.windows(2).any(|w| w == ["-crf", "24"]));
        assert!(p.encode.iter().any(|a| a.contains("format=yuv420p")));
        assert!(p.encode.windows(2).any(|w| w == ["-g", "60"]));
        assert_eq!(p.encode.last().map(String::as_str), Some("-an"));
        assert_eq!(p.extension, "mp4");
    }

    #[test]
    fn vaapi_plan_uploads_to_the_gpu() {
        let p = plan(&RecordingOptions { resolution: "720".into(), quality: "high".into(), ..opts() }, &input(1920, 1080), &caps(&["h264_vaapi"])).unwrap();
        assert_eq!(p.pre_input[0], "-vaapi_device");
        let vf = &p.encode[p.encode.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(vf, "scale=1280:720:flags=bicubic,format=nv12,hwupload");
        assert!(p.encode.windows(2).any(|w| w == ["-qp", "20"]));
        assert_eq!((p.width, p.height), (1280, 720));
    }

    #[test]
    fn unique_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = unique_path(dir.path(), "Demo: take/1", "mp4");
        assert_eq!(a.file_name().unwrap(), "Demo- take-1.mp4");
        std::fs::write(&a, b"x").unwrap();
        assert_eq!(unique_path(dir.path(), "Demo: take/1", "mp4").file_name().unwrap(), "Demo- take-1 (2).mp4");
        assert_eq!(unique_path(dir.path(), " .. ", "mkv").file_name().unwrap(), "Recording.mkv");
    }
}
